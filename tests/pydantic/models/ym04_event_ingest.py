# event ingestion: JSON lines -> a discriminated union via TypeAdapter, per-kind handlers updating pydantic-dataclass aggregates, errors collected from ValidationError.errors(), a generic result envelope
import json
from dataclasses import asdict, field
from typing import Annotated, Generic, Literal, TypeVar, Union

from pydantic import BaseModel, Field, TypeAdapter, ValidationError, model_validator
from pydantic.dataclasses import dataclass

T = TypeVar("T")


class Base(BaseModel):
    ts: int
    source: str = "web"


class Click(Base):
    kind: Literal["click"]
    target: str
    x: int = 0
    y: int = 0


class Purchase(Base):
    kind: Literal["purchase"]
    order_id: str
    amount: float

    @model_validator(mode="before")
    @classmethod
    def cents(cls, data):
        if "amount_cents" in data:
            data = {k: v for k, v in data.items() if k != "amount_cents"} | {"amount": data["amount_cents"] / 100}
            #@row data: definite={amount} absent={amount_cents} open
        return data


class Signup(Base):
    kind: Literal["signup"]
    user: str
    plan: str = "free"


Event = Annotated[Union[Click, Purchase, Signup], Field(discriminator="kind")]
EVENT = TypeAdapter(Event)


@dataclass
class Totals:
    clicks: int = 0
    revenue: float = 0.0
    signups: list[str] = field(default_factory=list)
    by_source: dict[str, int] = field(default_factory=dict)


class Result(BaseModel, Generic[T]):
    ok: bool
    data: T | None = None
    errors: list[dict] = []


def parse_line(line):
    raw = json.loads(line)
    ev = EVENT.validate_python(raw)              #@ possibly ts
    #@row ev: present={ts,source,kind,target,x,y,order_id,amount,user,plan} definite={ts,source,kind} closed
    return ev


def handle(ev, totals):
    totals.by_source[ev.source] = totals.by_source.get(ev.source, 0) + 1   #@ ok source
    if isinstance(ev, Click):
        totals.clicks += 1
        return ev.target                         #@ ok target
    if isinstance(ev, Purchase):
        totals.revenue += ev.amount              #@ ok amount
        return ev.order_id                       #@ ok order_id
    totals.signups.append(ev.user)               #@ ok user
    #@row ev: present={ts,source,kind,user,plan} definite={ts,source,kind,user,plan} closed
    return ev.plan


def target_of(ev):
    return ev.target                             #@ possibly target


def error_rows(e, lineno):
    rows = []
    for err in e.errors(include_url=False):
        row = {"line": lineno, "where": ".".join(str(p) for p in err["loc"]), "type": err["type"]}   #@ ok loc ; ok type
        #@row row: present={line,where,type} definite={line,where,type} closed
        rows.append(row)
    return rows


def ingest(lines):
    totals = Totals()
    errors = []
    for i, line in enumerate(lines, 1):
        try:
            ev = parse_line(line)
        except ValidationError as e:
            errors.extend(error_rows(e, i))
            continue
        handle(ev, totals)
    res = Result[Totals](ok=not errors, data=totals, errors=errors)
    #@row res: present={ok,data,errors} definite={ok,data,errors} closed
    return res


def report(res):
    d = res.model_dump()
    #@row d: present={ok,data,errors} definite={ok,data,errors} closed
    data = d["data"]
    #@row data: present={clicks,revenue,signups,by_source} definite={clicks,revenue,signups,by_source} closed
    return data["clicks"], data["revenue"]       #@ ok clicks ; ok revenue


LINES = [
    '{"kind": "click", "ts": 1, "target": "buy"}',
    '{"kind": "purchase", "ts": 2, "order_id": "o1", "amount": 9.5, "source": "app"}',
    '{"kind": "purchase", "ts": 3, "order_id": "o2", "amount_cents": 250}',
    '{"kind": "signup", "ts": 4, "user": "u1", "extra": true}',
]
BAD_LINES = [
    '{"kind": "click", "target": "x"}',
    '{"kind": "refund", "ts": 5}',
    '{"kind": "purchase", "ts": 6, "amount": 1}',
]

res = ingest(LINES)
ok = res.ok
#@value ok: exactly={True}
clicks = res.data.clicks
#@value clicks: exactly={1}
rev = res.data.revenue
#@value rev: exactly={12.0}
srcs = res.data.by_source
#@row srcs: present={web,app} definite={web,app} closed
agg = asdict(res.data)
#@row agg: present={clicks,revenue,signups,by_source} definite={clicks,revenue,signups,by_source} closed
bad = ingest(LINES[:1] + BAD_LINES)
bad_ok = bad.ok
#@value bad_ok: exactly={False}
n_err = len(bad.errors)
#@value n_err: exactly={3}
first_err = bad.errors[0]
#@row first_err: present={line,where,type} definite={line,where,type} closed
ftype = first_err["type"]
#@value ftype: exactly={"missing"}
tag_err = bad.errors[1]["type"]
#@value tag_err: exactly={"union_tag_invalid"}

if __name__ == "__main__":
    report(res)
    report(bad)
    for line in LINES:
        try:
            target_of(parse_line(line))
        except AttributeError:
            pass

#@schema json.loads@L63: always={kind,ts} sometimes={target,order_id,amount,amount_cents,user} only={kind,ts}

"""Validation layer: records become `Event` models; the failures are collected as
`Rejected` models carrying pydantic's error list."""
from collections import Counter
from typing import Optional

from pydantic import BaseModel, ValidationError, field_validator, model_validator


class Event(BaseModel):
    id: str
    user: str
    amount: float
    region: str = "unknown"
    ts: str
    source: str
    seq: int
    level: Optional[str] = None
    tags: list[str] = []

    @field_validator("user")
    @classmethod
    def norm_user(cls, v):
        v = v.strip().lower()
        if not v:
            raise ValueError("empty user")
        return v

    @field_validator("ts")
    @classmethod
    def iso_day(cls, v):
        if len(v) < 10 or v[4] != "-" or v[7] != "-":
            raise ValueError("bad timestamp")
        return v

    @model_validator(mode="after")
    def no_negative(self):
        if self.amount < 0 and self.level != "WARN":
            raise ValueError("negative amount")
        return self

    @property
    def day(self):
        return self.ts[:10]

    def key(self):
        return (self.source, self.id)


class Rejected(BaseModel):
    source: str
    seq: int
    errors: list[dict]

    def first(self):
        return self.errors[0] if self.errors else None


def describe_error(err):
    loc = ".".join(str(p) for p in err["loc"])
    return {"loc": loc or "__root__", "type": err["type"], "msg": err["msg"]}


def validate_records(records, strict=False):
    good, bad = [], []
    seen = set()
    for rec in records:
        try:
            ev = Event.model_validate(rec)
        except ValidationError as e:
            bad.append(Rejected(source=rec.get("source", "?"), seq=rec.get("seq", -1), errors=[describe_error(x) for x in e.errors()]))
            if strict:
                break
            continue
        if ev.key() in seen:
            continue
        seen.add(ev.key())
        good.append(ev)
    return good, bad


def error_summary(rejected):
    counts = Counter()
    for r in rejected:
        for err in r.errors:
            counts[(r.source, err["type"])] += 1
    return {f"{src}:{typ}": n for (src, typ), n in sorted(counts.items())}

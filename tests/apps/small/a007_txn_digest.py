# Transaction digest: re named groups (an optional group), datetime.strptime, Decimal quantize, a NamedTuple built in a helper, an lru_cache'd rate table, functools.partial / reduce with a callback defined elsewhere, options from JSON in os.environ
"""Hypothesis: a NamedTuple built from a match's groupdict in one function, converted by a
partial of another (the keyword fixed by the partial reaches the callee) and folded by reduce
through a lambda that calls a third function, gives a dict whose keys are the literal days of
literal log lines and whose values are exact Decimal texts; lines from input give an open dict
(keys come from data). The optional group is None, not a missing key."""
import json
import os
import re
from datetime import datetime
from decimal import ROUND_HALF_EVEN, Decimal
from functools import lru_cache, partial, reduce
from typing import NamedTuple, Optional

LINE = re.compile(r"(?P<ts>\d{4}-\d\d-\d\d \d\d:\d\d) (?P<level>[A-Z]+) (?P<cur>[A-Z]{3}) (?P<amount>-?\d+\.\d\d)(?: ref=(?P<ref>\w+))?$")

LOG = """2024-05-01 09:15 INFO EUR 12.50 ref=A1
2024-05-01 17:40 WARN USD 10.00
garbage line
2024-05-02 08:05 INFO NOK 200.00 ref=Z9
2024-05-02 23:59 ERROR EUR -3.25
"""


class Txn(NamedTuple):
    at: datetime
    level: str
    amount: Decimal
    currency: str
    ref: Optional[str] = None


@lru_cache(maxsize=None)
def rate(cur):
    return {"EUR": Decimal("1"), "USD": Decimal("0.90"), "NOK": Decimal("0.085")}.get(cur, Decimal("0"))


def parse(line):
    m = LINE.match(line)
    if m is None:
        return None
    g = m.groupdict()
    #@row g: present={ts,level,cur,amount,ref} definite={ts,level,cur,amount,ref} closed
    at = datetime.strptime(g["ts"], "%Y-%m-%d %H:%M")   #@ ok ts
    return Txn(at, g["level"], Decimal(g["amount"]), g["cur"], g["ref"])


def to_eur(t, places):
    q = Decimal(1).scaleb(-places)
    return (t.amount * rate(t.currency)).quantize(q, rounding=ROUND_HALF_EVEN)


def fold(acc, t, convert):
    day = t.at.date().isoformat()
    acc[day] = acc.get(day, Decimal("0")) + convert(t)
    return acc


def daily(txns, places=2):
    conv = partial(to_eur, places=places)
    return reduce(lambda acc, t: fold(acc, t, conv), txns, {})


def make_report(text, opts):
    places = opts["places"]
    wanted = opts.get("levels")
    txns = [t for t in map(parse, text.splitlines()) if t is not None]
    if wanted is not None:
        txns = [t for t in txns if t.level in wanted]
    out = {"days": {d: str(v) for d, v in daily(txns, places).items()}, "n": len(txns)}
    refs = {t.ref: t for t in txns if t.ref}
    if refs:
        out["refs"] = refs
    return out
#@schema param make_report.opts: always={places} only={places}


def ref_amount(refs):
    return refs["A1"].amount                     #@ possibly A1


if __name__ == "__main__":
    t0 = parse("2024-05-01 17:40 WARN USD 10.00")
    #@row t0: present={at,level,amount,currency,ref} definite={at,level,amount,currency,ref} closed
    r0 = t0.ref                                  #@value r0: exactly={None}
    e0 = str(to_eur(t0, 2))                      #@value e0: exactly={"9.00"}
    e1 = str(partial(to_eur, places=1)(t0))      #@value e1: exactly={"9.0"}
    hour = t0.at.hour                            #@value hour: exactly={17}
    none = parse("garbage")                      #@value none: exactly={None}

    rep = make_report(LOG, {"places": 2})
    #@row rep: present={days,n,refs} definite={days,n,refs} closed
    days = rep["days"]
    #@row days: present={"2024-05-01","2024-05-02"} definite={"2024-05-01","2024-05-02"} closed
    d1 = days["2024-05-01"]                      #@value d1: exactly={"21.50"}
    d2 = days["2024-05-02"]                      #@value d2: exactly={"13.75"}
    n = rep["n"]                                 #@value n: exactly={4}
    refs = rep["refs"]
    #@row refs: present={A1,Z9} definite={A1,Z9} closed
    a1 = str(ref_amount(refs))                   #@value a1: exactly={"12.50"}

    warn = make_report(LOG, {"places": 0, "levels": ["WARN", "ERROR"]})
    #@row warn: present={days,n} definite={days,n} absent={refs} closed
    w1 = warn["days"]["2024-05-01"]              #@value w1: exactly={"9"}
    w2 = warn["days"]["2024-05-02"]              #@value w2: exactly={"-3"}
    try:
        ref_amount({"Z9": t0})
    except KeyError:
        pass

    opts = json.loads(os.environ.get("DIGEST_OPTS", '{"places": 3}'))
    extra = make_report(os.environ.get("DIGEST_LOG", ""), opts)
    #@row extra: present={days,n,refs} definite={days,n} closed
    xdays = extra["days"]
    #@row xdays: present={} definite={} open

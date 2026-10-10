"""Hypothesis: try/except/else/finally adds the else key only on the path where the body did not
raise and the finally key on every path; `raise ParseError(...) from e` keeps the original
exception as __cause__ and the custom attributes reach a handler two calls up; a
contextlib.suppress block stops at the failing read (later stores in the block skipped, the
statement after the block runs). Exact for literal records; a record from input gives the join
of the else and except rows."""
import contextlib
import json
import os


class ParseError(Exception):
    def __init__(self, field, reason):
        super().__init__(f"{field}: {reason}")
        self.field = field
        self.reason = reason


def parse_int(rec, field):
    try:
        return int(rec[field])
    except KeyError as e:
        raise ParseError(field, "missing") from e
    except ValueError as e:
        raise ParseError(field, "bad") from e


def load(rec):
    out = {"seen": True}
    try:
        n = parse_int(rec, "qty")
    except ParseError as err:
        out["error"] = err.field + "/" + err.reason
        out["cause"] = type(err.__cause__).__name__
    else:
        out["qty"] = n
    finally:
        out["done"] = 1
    return out


def outer(recs):
    total = 0
    for r in recs:
        total += parse_int(r, "n")
    return total


def safe_total(recs):
    try:
        return {"total": outer(recs)}
    except ParseError as e:
        return {"failed": e.field, "msg": str(e)}


def tolerant(d):
    out = {}
    with contextlib.suppress(KeyError):
        out["a"] = d["a"]
        out["b"] = d["b"]
    out["end"] = True
    return out


if __name__ == "__main__":
    r1 = load({"qty": "3"})
    #@row r1: present={seen,qty,done} definite={seen,qty,done} absent={error,cause} closed
    q1 = r1["qty"]                               #@value q1: exactly={3}
    r2 = load({})
    #@row r2: present={seen,error,cause,done} definite={seen,error,cause,done} absent={qty} closed
    e2 = r2["error"] + " " + r2["cause"]         #@value e2: exactly={"qty/missing KeyError"}
    r3 = load({"qty": "x"})
    c3 = r3["cause"]                             #@value c3: exactly={"ValueError"}
    t1 = safe_total([{"n": "1"}, {"n": "2"}])
    #@row t1: present={total} definite={total} closed
    tot = t1["total"]                            #@value tot: exactly={3}
    t2 = safe_total([{"n": "1"}, {}])
    #@row t2: present={failed,msg} definite={failed,msg} closed
    msg = t2["msg"]                              #@value msg: exactly={"n: missing"}
    u1 = tolerant({"a": 1})
    #@row u1: present={a,end} definite={a,end} absent={b} closed
    u2 = tolerant({"b": 2})
    #@row u2: present={end} definite={end} absent={a,b} closed
    r4 = load(json.loads(os.environ.get("REC", '{"qty": "7", "extra": 1}')))
    #@row r4: present={seen,qty,error,cause,done} definite={seen,done} closed

"""Hypothesis: json.loads of a literal text that does not parse raises JSONDecodeError with
CPython's msg / pos / lineno / colno and str(e); NaN and Infinity parse (no raise claimed);
a generator that yields in an except handler keeps its items' positions, so the records a
consumer tags in order are exact."""
import json


def records(lines, errors):
    for n, line in enumerate(lines, 1):
        try:
            obj = json.loads(line)
        except json.JSONDecodeError as e:
            errors.append(n)
            yield None
            continue
        yield {"id": obj["id"]}


def load(lines):
    out = []
    errs = []
    for i, rec in enumerate(records(lines, errs)):
        if rec is None:
            continue
        rec["seq"] = i
        out.append(rec)
    return out, errs


recs, errs = load(['{"id": 10}', "not json", '{"id": 12}'])
r1 = recs[1]
#@row r1: present={id,seq} definite={id,seq} closed
seq1 = r1["seq"]                                  #@value seq1: exactly={2}
n_recs = len(recs)                                #@value n_recs: exactly={2}

try:
    json.loads('{"a": 1,\n "b" 2}')
    msg = pos = line = col = text = None
except ValueError as e:
    msg = e.msg                                   #@value msg: exactly={"Expecting ':' delimiter"}
    pos = e.pos                                   #@value pos: exactly={14}
    line = e.lineno                               #@value line: exactly={2}
    col = e.colno                                 #@value col: exactly={6}
    text = str(e)                                 #@value text: exactly={"Expecting ':' delimiter: line 2 column 6 (char 14)"}

try:
    v = json.loads("[1, 2,]")
    t = "ok"
except json.JSONDecodeError as e:
    t = e.msg                                     #@value t: exactly={"Illegal trailing comma before end of array"}

nan_ok = "no"
try:
    json.loads("[NaN, Infinity]")
    nan_ok = "yes"
except ValueError:
    pass
nan_res = nan_ok                                  #@value nan_res: includes={"yes"}

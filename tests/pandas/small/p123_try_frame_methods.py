"""Frame code under a try: a frame from records (columns= selecting among their keys),
reset_index(drop=True), copy() and the .str accessor do not raise, so a handler of an
unrelated error never runs; a column read that may fail starts the handler from before the
read (the column is not taken as there afterwards); a tuple key of parameters (a registry
of (kind, alias) entries) is exact per call site."""
import io
import sys

import pandas as pd


class LookupFailed(KeyError):
    pass


ALIASES = {("source", "comma"): "csv", ("source", "ndjson"): "jsonl"}


def canonical(kind, name):
    return ALIASES.get((kind, name), name)


rows = [{"id": "1", "ts": "2024-03-01T10:00", "amount": 2.0}, {"id": "2", "ts": "2024-03-02T11:00", "amount": 0.5}]
state = "ok"
try:
    df = pd.DataFrame(rows, columns=["id", "ts", "amount"])
    df["day"] = df["ts"].str.slice(0, 10)
    df = df[df["amount"] >= 1.0].reset_index(drop=True).copy()
except LookupFailed:
    state = "failed"
res = state                                       #@value res: exactly={"ok"}
#@row df: present={id,ts,amount,day} definite={id,ts,amount,day} closed

opened = pd.read_csv(io.StringIO(sys.argv[1]))
seen = "yes"
try:
    opened["comments"].str.len()
except KeyError:
    seen = "no"
seen_v = seen                                     #@value seen_v: exactly={"yes","no"}
has = "comments" in opened.columns                #@value has: unknown

c1 = canonical("source", "comma")                 #@value c1: exactly={"csv"}
c2 = canonical("source", "csv")                   #@value c2: exactly={"csv"}

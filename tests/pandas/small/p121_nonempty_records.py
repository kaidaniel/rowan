"""A list of records of unknown length tested `if not rows:` before building a frame: on the
other branch it has a first record, so a frame from a comprehension over it has the keys every
record surely has as definite columns (a key some records have stays present-only); the empty
branch's frame names its columns."""
import json
import os

import pandas as pd


def load(raw):
    out = []
    for r in raw:
        rec = {"sensor": r["id"], "value": float(r["v"])}
        if "note" in r:
            rec["note"] = r["note"]
        out.append(rec)
    return out


def to_frame(rows):
    if not rows:
        return pd.DataFrame(columns=["sensor", "value"])
    df = pd.DataFrame([dict(r) for r in rows])
    first = rows[0]
    _ = 0                                        #@row first: present={sensor,value,note} definite={sensor,value} closed
    return df


if __name__ == "__main__":
    raw = json.loads(os.environ.get("P121_JSON", '[{"id": "a", "v": "1"}, {"id": "b", "v": 2, "note": "x"}]'))
    df = to_frame(load(raw))
    #@row df: present={sensor,value,note} definite={sensor,value} closed
    empty = to_frame([])
    #@row empty: present={sensor,value} definite={sensor,value} closed
    s = df["sensor"]                             #@ ok sensor

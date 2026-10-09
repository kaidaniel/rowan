# to_json(orient=…) round trips: records / columns keep the columns as keys, split / table have fixed keys, read_json gets the columns back
import io
import json

import pandas as pd


def roundtrip(df):
    recs = json.loads(df.to_json(orient="records"))
    first = recs[0]
    #@row first: present={a,b,c} definite={a,b} closed
    cols = json.loads(df.to_json(orient="columns"))
    #@row cols: present={a,b,c} definite={a,b} closed
    split = json.loads(df.to_json(orient="split"))
    #@row split: present={columns,index,data} definite={columns,index,data} closed
    table = json.loads(df.to_json(orient="table"))
    #@row table: present={schema,data} definite={schema,data} closed
    back = pd.read_json(io.StringIO(df.to_json(orient="records")))
    #@row back: present={a,b,c} definite={a,b} closed
    back["b"]                                    #@ ok b
    return first


def other(first):
    return first["z"]                            #@ missing z


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for f in (pd.DataFrame({"a": [1, 2], "b": ["x", None]}),
              pd.DataFrame({"a": [3], "b": ["y"], "c": [1.5]})):
        try:
            other(roundtrip(f))
        except KeyError as e:
            kept.append(e)

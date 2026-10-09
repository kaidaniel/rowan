# to_json / read_json(StringIO) round trips keep the columns; json.loads of the text gives dicts keyed by columns
import io
import json

import pandas as pd


def roundtrip(df):
    back = pd.read_json(io.StringIO(df.to_json()))       #@row back: present={a,b,c} definite={a} closed
    recs = pd.read_json(io.StringIO(df.to_json(orient="records")), orient="records")   #@row recs: present={a,b,c} definite={a} closed
    split = pd.read_json(io.StringIO(df.to_json(orient="split")), orient="split")      #@row split: present={a,b,c} definite={a} closed
    r0 = json.loads(df.to_json(orient="records"))[0]     #@row r0: present={a,b,c} definite={a} closed
    cols = json.loads(df.to_json())                      #@row cols: present={a,b,c} definite={a} closed
    table = json.loads(df.to_json(orient="table"))       #@row table: present={schema,data} definite={schema,data} closed
    return back["c"]                                     #@ ok c ; missing c


if __name__ == "__main__":
    caught = []  # keeps each raised exception alive
    f1 = pd.DataFrame({"a": [1, 2], "b": ["x", "y"]})
    f2 = pd.DataFrame({"a": [3], "c": [4.5]})
    f3 = pd.DataFrame({"a": [5, 6], "b": ["p", "q"], "c": [0.0, 1.0]})
    for frame in (f1, f2, f3):
        try:
            roundtrip(frame)
        except KeyError as e:
            caught.append(e)

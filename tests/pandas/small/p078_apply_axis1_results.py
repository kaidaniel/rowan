# apply(axis=1): a returned Series makes columns from its keys; a dict gives a Series of dicts unless result_type="expand"
import pandas as pd


def summarise(row):
    return pd.Series({"total": row["qty"] * 2, "big": row["qty"] > 1})   #@ ok qty


def as_dict(row):
    return {"total": row["qty"] * 2}


def maybe_big(row):
    d = {"id": row["id"]}                                #@ ok id
    if row["qty"] > 1:
        d["big"] = True
    return pd.Series(d)


def as_series(df):
    out = df.apply(summarise, axis=1)                    #@row out: present={total,big} definite={total,big} closed
    return out["qty"]                                    #@ missing qty


def dicts(df):
    s = df.apply(as_dict, axis=1)
    first = s.iloc[0]                                    #@row first: present={total} definite={total} closed
    out = df.apply(as_dict, axis=1, result_type="expand")   #@row out: present={total} definite={total} closed
    df = df.assign(score=df.apply(as_dict, axis=1, result_type="expand")["total"])   #@ ok total
    #@row df: present={id,qty,note,score} definite={id,qty,score} closed
    return s["total"]                                    #@ missing total


def flags(df):
    out = df.apply(maybe_big, axis=1)                    #@row out: present={id,big} definite={id} closed
    return out["big"]                                    #@ possibly big


if __name__ == "__main__":
    caught = []  # keeps each raised exception alive
    a = pd.DataFrame({"id": [1, 2], "qty": [1, 2]})
    b = pd.DataFrame({"id": [3, 4], "qty": [0, 1], "note": ["x", "y"]})
    for frame in (a, b):
        for f in (as_series, dicts, flags):
            try:
                f(frame)
            except KeyError as e:
                caught.append(e)

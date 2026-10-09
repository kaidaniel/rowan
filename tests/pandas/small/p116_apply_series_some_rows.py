# apply(axis=1) returning a pd.Series with a key added on some rows only: the result's columns are the union over the rows
import pandas as pd


def tag(row):
    out = {"id": row["id"]}                      #@ ok id
    if row["v"] > 0:
        out["pos"] = True
    return pd.Series(out)                        #@row out: present={id,pos} definite={id} closed


def run(df):
    res = df.apply(tag, axis=1)
    #@row res: present={id,pos} definite={id} absent={v} closed
    res["id"]                                    #@ ok id
    return res


def pos(res):
    return res["pos"]                            #@ possibly pos


def v(res):
    return res["v"]                              #@ missing v


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for f in (pd.DataFrame({"id": [1, 2], "v": [1, -1]}),
              pd.DataFrame({"id": [3], "v": [-2]}),
              pd.DataFrame({"id": [4, 5], "v": [3, 4], "w": [0, 0]})):
        res = run(f)
        for fn in (pos, v):
            try:
                fn(res)
            except KeyError as e:
                kept.append(e)

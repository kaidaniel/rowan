# explode: list cells become rows, the columns stay the same (one column, a list of columns, ignore_index); a Series explode turned back into a frame
import io

import pandas as pd

CSV = "id,tags,memo\n1,a;b,x\n2,c,y\n"


def one(df):
    out = df.explode("tags")                       #@ ok tags
    #@row out: present={id,tags,n} definite={id,tags} closed
    out["id"]                                      #@ ok id
    out["tag"]                                     #@ missing tag


def two(df):
    out = df.explode(["a", "b"], ignore_index=True)
    #@row out: present={k,a,b} definite={k,a,b} closed
    return out


def series(df):
    s = df["tags"].explode()                       #@ ok tags
    f = s.to_frame()
    #@row f: present={tags} definite={tags} absent={id} closed
    f2 = s.reset_index()
    #@row f2: present={index,tags} definite={index,tags} absent={id} closed
    return f2


raw = pd.read_csv(io.StringIO(CSV))
raw["tags"] = raw["tags"].str.split(";")           #@ input tags
ex = raw.explode("tags")
#@row ex: definite={tags} open
ex["id"]                                           #@ input id
#@schema read_csv@L31: always={tags,id} only={tags,id}

if __name__ == "__main__":
    for f in (pd.DataFrame({"id": [1, 2], "tags": [["a", "b"], ["c"]]}),
              pd.DataFrame({"id": [1], "tags": [[]], "n": [0]})):
        try:
            one(f)
        except KeyError:
            pass
    two(pd.DataFrame({"k": [1, 2], "a": [[1, 2], [3]], "b": [["x", "y"], ["z"]]}))
    two(pd.DataFrame({"k": [1], "a": [[1]], "b": [["x"]]}))
    series(pd.DataFrame({"id": [1, 2], "tags": [["a", "b"], ["c"]]}))
    series(pd.DataFrame({"id": [3], "tags": [["d"]]}))

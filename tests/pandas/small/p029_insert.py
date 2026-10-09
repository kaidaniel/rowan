# DataFrame.insert(loc, column, value): adds in place and returns None (position does not matter for keys), allow_duplicates, insert on one branch
import io

import pandas as pd

CSV = "id,score,memo\n1,3,a\n2,5,b\n"


def put_first(df):
    df.insert(0, "rank", df["score"].rank())       #@ ok score
    n = len(df.columns)                            #@row df: present={id,score,rank,extra} definite={id,score,rank} closed
    return df["rank"]                              #@ ok rank


def put_last(df):
    res = df.insert(len(df.columns), "flag", True)
    #@value res: exactly={None}
    #@row df: present={id,score,flag} definite={id,score,flag} closed
    return df


def twice(df):
    df.insert(1, "dup", 0)
    df.insert(1, "dup", 1, allow_duplicates=True)
    n = len(df.columns)                            #@row df: present={id,score,dup} definite={id,score,dup} closed
    return df


def maybe_insert(df, add):
    if add:
        df.insert(0, "note", "")
    n = len(df.columns)                            #@row df: present={id,score,note} definite={id,score} closed
    return df["note"]                              #@ possibly note


raw = pd.read_csv(io.StringIO(CSV))
raw.insert(0, "z", raw["score"] * 2)              #@ input score
#@row raw: definite={score,z} open
raw["z"]                                           #@ ok z
raw["id"]                                          #@ input id
#@schema read_csv@L36: always={score,id} only={score,id}

if __name__ == "__main__":
    put_first(pd.DataFrame({"id": [1, 2], "score": [3, 1]}))
    put_first(pd.DataFrame({"id": [1], "score": [3], "extra": [0]}))
    put_last(pd.DataFrame({"id": [1], "score": [2]}))
    put_last(pd.DataFrame({"id": [], "score": []}))
    twice(pd.DataFrame({"id": [1], "score": [2]}))
    twice(pd.DataFrame({"id": [3, 4], "score": [5, 6]}))
    for add in (True, False):
        try:
            maybe_insert(pd.DataFrame({"id": [1], "score": [2]}), add)
        except KeyError:
            pass

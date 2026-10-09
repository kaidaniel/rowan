# first() / last(): the non-key columns indexed by key; nth(0) / head(1) filter rows and keep every column
import pandas as pd

E1 = pd.DataFrame({"user": ["u", "u", "v"], "ts": [1, 2, 3], "page": ["a", "b", "c"]})
E2 = pd.DataFrame({"user": ["w"], "ts": [4], "page": ["d"], "ref": ["x"]})


def firsts(df):
    f = df.groupby("user").first()
    #@row f: present={ts,page,ref} definite={ts,page} absent={user} closed
    l = df.groupby("user").last()
    #@row l: present={ts,page,ref} definite={ts,page} absent={user} closed
    n = df.groupby("user").nth(0)
    #@row n: present={user,ts,page,ref} definite={user,ts,page} closed
    n["user"]                                            #@ ok user
    h = df.sort_values("ts").groupby("user").head(1)
    #@row h: present={user,ts,page,ref} definite={user,ts,page} closed
    return f


def user(f):
    return f["user"]                                     #@ missing user


def ref(f):
    return f["ref"]                                      #@ possibly ref


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for df in (E1, E2):
        try:
            user(firsts(df))
        except KeyError as e:
            kept.append(e)
    ref(firsts(E2))
    try:
        ref(firsts(E1))
    except KeyError as e:
        kept.append(e)

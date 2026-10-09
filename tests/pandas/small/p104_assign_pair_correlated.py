# two columns always added together on one branch (assign(a=…, b=…)): `"a" in df` decides b too
import pandas as pd


def enrich(df, flag):
    if flag:
        df = df.assign(a=1, b=2)
    if "a" in df:                                #@row df: present={k,a,b} definite={k} closed
        v = df["b"]                              #@ ok b
        #@row df: present={k,a,b} definite={k,a,b} closed
    else:
        v = df.get("b")                          #@value v: exactly={None}
        #@row df: present={k} definite={k} absent={a,b} closed
    has_b = "b" in df.columns                    #@value has_b: exactly={True,False}
    return df


def read_a(df):
    return df["a"]                               #@ possibly a


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for flag in (True, False):
        for f in (pd.DataFrame({"k": [1, 2]}), pd.DataFrame({"k": []})):
            out = enrich(f, flag)
            try:
                read_a(out)
            except KeyError as e:
                kept.append(e)

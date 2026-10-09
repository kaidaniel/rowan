# an `in` test on a name that holds one of two frames: the test picks the frame, so the reads behind it are decided
import pandas as pd

A = pd.DataFrame({"a": [1, 2], "s": [3, 4]})
B = pd.DataFrame({"b": [5], "s": [6]})


def pick(flag):
    df = A if flag else B
    #@row df: present={a,b,s} definite={s} closed
    has = "a" in df.columns                      #@value has: exactly={True,False}
    if "a" in df.columns:
        v = df["a"]                              #@ ok a
        #@row df: present={a,s} definite={a,s} absent={b} closed
    else:
        v = df["b"]                              #@ ok b
        #@row df: present={b,s} definite={b,s} absent={a} closed
    df["s"]                                      #@ ok s
    return df


def read_b(df):
    return df["b"]                               #@ possibly b


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for flag in (True, False, True):
        got = pick(flag)
        try:
            read_b(got)
        except KeyError as e:
            kept.append(e)

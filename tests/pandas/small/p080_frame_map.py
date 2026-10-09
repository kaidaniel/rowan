# DataFrame.map (elementwise, applymap's successor) keeps the columns; Series.map / where feed new columns
import pandas as pd


def tenfold(v):
    return v * 10


def scaled(df):
    out = df.map(tenfold)                                #@row out: present={a,b,c} definite={a} closed
    kept = df.map(tenfold, na_action="ignore")           #@row kept: present={a,b,c} definite={a} closed
    w = out.where(out > 10)                              #@row w: present={a,b,c} definite={a} closed
    return out["a"]                                      #@ ok a


def labelled(df):
    df = df.copy()
    df["label"] = df["a"].map({1: "one", 2: "two"})      #@ ok a
    #@row df: present={a,b,c,label} definite={a,label} closed
    df["b2"] = df["b"].map(tenfold)                      #@ ok b ; missing b
    return df


def no_label(df):
    return df.map(tenfold)["label"]                      #@ missing label


if __name__ == "__main__":
    caught = []  # keeps each raised exception alive
    f1 = pd.DataFrame({"a": [1, 2], "b": [3, 4]})
    f2 = pd.DataFrame({"a": [2, 3], "c": [5, 6]})
    f3 = pd.DataFrame({"a": [1], "b": [0], "c": [9]})
    for frame in (f1, f2, f3):
        scaled(frame)
        try:
            labelled(frame)
        except KeyError as e:
            caught.append(e)
        try:
            no_label(frame)
        except KeyError as e:
            caught.append(e)

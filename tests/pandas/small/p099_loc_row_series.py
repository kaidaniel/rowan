# One row as a Series (iloc / loc / xs): its keys are the columns; loc with a column list narrows them; items() pairs
import pandas as pd


def first_row(df):
    r = df.iloc[0]                                       #@row r: present={a,b,c} definite={a} closed
    r["a"]                                               #@ ok a
    last = df.iloc[-1].to_dict()                         #@row last: present={a,b,c} definite={a} closed
    x = df.xs(df.index[0])                               #@row x: present={a,b,c} definite={a} closed
    sub = df.loc[df.index[0], ["a"]]                     #@row sub: present={a} definite={a} absent={b,c} closed
    pairs = dict(df.items())                             #@row pairs: present={a,b,c} definite={a} closed
    r.get("c")
    return r["c"]                                        #@ ok c ; missing c


def by_label(df):
    row = df.loc["k1"]                                   #@ ok k1
    #@row row: present={a,b,c} definite={a} closed
    return row["z"]                                      #@ missing z


if __name__ == "__main__":
    caught = []  # keeps each raised exception alive
    f1 = pd.DataFrame({"a": [1, 2], "b": [3, 4]}, index=["k1", "k2"])
    f2 = pd.DataFrame({"a": [5], "c": [6]}, index=["k1"])
    for frame in (f1, f2):
        try:
            first_row(frame)
        except KeyError as e:
            caught.append(e)
        try:
            by_label(frame)
        except KeyError as e:
            caught.append(e)

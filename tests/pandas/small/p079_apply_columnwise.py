# apply(f) column by column: a reducer gives a Series indexed by the columns, a transform keeps the columns
import pandas as pd


def span(col):
    return col.max() - col.min()


def double(col):
    return col * 2


def describe_col(col):
    return col.describe()


def spans(df):
    s = df.apply(span)                                   #@row s: present={a,b,c} definite={a} closed
    return s["a"]                                        #@ ok a


def doubled(df):
    out = df.apply(double)                               #@row out: present={a,b,c} definite={a} closed
    tr = df.transform(double)                            #@row tr: present={a,b,c} definite={a} closed
    out["a"]                                             #@ ok a
    return out["c"]                                      #@ possibly c


def described(df):
    stats = df.apply(describe_col)                       #@row stats: present={a,b,c} definite={a} closed
    t = stats.T                                          #@row t: present={count,mean,std,min,"25%","50%","75%",max} definite={count,mean,std,min,"25%","50%","75%",max} closed
    return t["total"]                                    #@ missing total


if __name__ == "__main__":
    caught = []  # keeps each raised exception alive
    f1 = pd.DataFrame({"a": [1, 5], "b": [2.0, 3.0]})
    f2 = pd.DataFrame({"a": [4, 1], "c": [7, 9]})
    f3 = pd.DataFrame({"a": [0, 2], "b": [1.0, 1.5], "c": [3, 3]})
    for frame in (f1, f2, f3):
        spans(frame)
        try:
            doubled(frame)
        except KeyError as e:
            caught.append(e)
        try:
            described(frame)
        except KeyError as e:
            caught.append(e)

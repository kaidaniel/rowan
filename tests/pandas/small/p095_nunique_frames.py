# nunique on frames: a Series over the columns; selecting columns by it gives a data-dependent subset of them
import pandas as pd


def uniques(df):
    n = df.nunique()                                     #@row n: present={a,b,c} definite={a} closed
    d = n.to_dict()                                      #@row d: present={a,b,c} definite={a} closed
    nf = n.to_frame("n")                                 #@row nf: present={n} definite={n} absent={a} closed
    varying = df.loc[:, df.nunique() > 1]                #@row varying: present={a,b,c} closed
    wide = df.assign(distinct=df.nunique(axis=1))        #@row wide: present={a,b,c,distinct} definite={a,distinct} closed
    return varying["a"]                                  #@ possibly a


def count_of(df):
    return df.nunique()["b"]                             #@ ok b ; missing b


if __name__ == "__main__":
    caught = []  # keeps each raised exception alive
    f1 = pd.DataFrame({"a": [1, 2], "b": [3, 3]})
    f2 = pd.DataFrame({"a": [7, 7], "c": [1, 2]})
    f3 = pd.DataFrame({"a": [1, 1, 2], "b": [0, 1, 2], "c": [5, 5, 5]})
    for frame in (f1, f2, f3):
        try:
            uniques(frame)
        except KeyError as e:
            caught.append(e)
        try:
            count_of(frame)
        except KeyError as e:
            caught.append(e)

# column projection: df[[...]], df.loc[:, [...]], df.iloc[:, :2], df[[]] and a one-item list
import pandas as pd


def project(df):
    sub = df[["a", "b"]]
    #@row sub: present={a,b} definite={a,b} absent={c,d} closed
    sub["a"]                                     #@ ok a
    one = df[["b"]]
    #@row one: present={b} definite={b} closed
    via_loc = df.loc[:, ["b", "a"]]
    #@row via_loc: present={a,b} definite={a,b} closed
    sub["c"]                                     #@ missing c


def by_position(df):
    first2 = df.iloc[:, :2]
    #@row first2: present={a,b,x} definite={a} closed
    first2["a"]                                  #@ ok a
    none = df[[]]
    #@row none: present={} closed
    none["a"]                                    #@ missing a


frame = pd.DataFrame({"k": [1], "a": [2], "b": [3]})
picked = frame[["k", "b"]]                       #@row picked: present={k,b} definite={k,b} absent={a} closed
picked["k"]                                      #@ ok k
mid = frame.iloc[:, 1:]                          #@row mid: present={a,b} definite={a,b} absent={k} closed
mid["b"]                                         #@ ok b

if __name__ == "__main__":
    caught = []
    for df in [pd.DataFrame({"a": [1], "b": [2], "c": [3]}), pd.DataFrame({"b": [1, 2], "a": [3, 4], "d": [5, 6]}), pd.DataFrame({"a": [], "b": []})]:
        try:
            project(df)
        except KeyError as e:
            caught.append(e)
    for df in [pd.DataFrame({"a": [1], "b": [2], "c": [3]}), pd.DataFrame({"a": [1], "x": [2]})]:
        try:
            by_position(df)
        except KeyError as e:
            caught.append(e)

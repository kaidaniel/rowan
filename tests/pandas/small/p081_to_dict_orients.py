# to_dict orients: dict / list / series keyed by columns, index keyed by row labels, split / tight fixed keys
import pandas as pd


def orients(df):
    d = df.to_dict()                                     #@row d: present={a,b,c} definite={a} closed
    lists = df.to_dict("list")                           #@row lists: present={a,b,c} definite={a} closed
    series = df.to_dict("series")                        #@row series: present={a,b,c} definite={a} closed
    col = d["a"]                                         #@row col: present={r1,r2,r3} definite={r1} closed
    by_row = df.to_dict("index")                         #@row by_row: present={r1,r2,r3} definite={r1} absent={a} closed
    inner = by_row["r1"]                                 #@row inner: present={a,b,c} definite={a} closed
    sp = df.to_dict("split")                             #@row sp: present={index,columns,data} definite={index,columns,data} closed
    tight = df.to_dict("tight")                          #@row tight: present={index,columns,data,index_names,column_names} definite={index,columns,data,index_names,column_names} closed
    return d["c"]                                        #@ possibly c


def row_label(df):
    return df.to_dict("index")["a"]                      #@ missing a


if __name__ == "__main__":
    caught = []  # keeps each raised exception alive
    f1 = pd.DataFrame({"a": [1, 2], "b": [3, 4]}, index=["r1", "r2"])
    f2 = pd.DataFrame({"a": [5, 6], "c": [7, 8]}, index=["r1", "r3"])
    for frame in (f1, f2):
        try:
            orients(frame)
        except KeyError as e:
            caught.append(e)
        try:
            row_label(frame)
        except KeyError as e:
            caught.append(e)

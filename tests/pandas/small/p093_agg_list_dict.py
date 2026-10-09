# DataFrame.agg: a list keeps the columns (index = the functions), a dict selects them, a string reduces to a Series
import pandas as pd


def aggregated(df):
    out = df.agg(["sum", "max"])                         #@row out: present={a,b,c} definite={a,b} closed
    t = out.T                                            #@row t: present={sum,max} definite={sum,max} closed
    by = df.agg({"a": "sum", "b": "max"})                #@row by: present={a,b} definite={a,b} absent={c} closed
    multi = df.agg({"a": ["sum", "min"], "b": ["max"]})  #@row multi: present={a,b} definite={a,b} absent={c} closed
    mt = multi.T                                         #@row mt: present={sum,min,max} definite={sum,min,max} closed
    one = df.agg("sum")                                  #@row one: present={a,b,c} definite={a,b} closed
    return multi["c"]                                    #@ missing c


def named(df):
    res = df.agg(total=("a", "sum"), top=("b", "max"))   #@row res: present={a,b} definite={a,b} closed
    return res.T["total"]                                #@ ok total


if __name__ == "__main__":
    caught = []  # keeps each raised exception alive
    f1 = pd.DataFrame({"a": [1, 2], "b": [3, 4]})
    f2 = pd.DataFrame({"a": [5, 6], "b": [7.0, 8.0], "c": [9, 10]})
    for frame in (f1, f2):
        try:
            aggregated(frame)
        except KeyError as e:
            caught.append(e)
        named(frame)

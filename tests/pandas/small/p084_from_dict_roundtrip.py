# DataFrame.from_dict / from_records / DataFrame(**split): round trips through to_dict keep the columns
import pandas as pd


def roundtrip(df):
    back = pd.DataFrame.from_dict(df.to_dict())          #@row back: present={a,b,c} definite={a} closed
    lists = pd.DataFrame.from_dict(df.to_dict("list"))   #@row lists: present={a,b,c} definite={a} closed
    rows = pd.DataFrame.from_dict(df.to_dict("index"), orient="index")   #@row rows: present={a,b,c} definite={a} closed
    tight = pd.DataFrame.from_dict(df.to_dict("tight"), orient="tight")  #@row tight: present={a,b,c} definite={a} closed
    split = pd.DataFrame(**df.to_dict("split"))          #@row split: present={a,b,c} definite={a} closed
    recs = pd.DataFrame.from_records(df.to_dict("records"))   #@row recs: present={a,b,c} definite={a} closed
    return recs["b"]                                     #@ ok b ; missing b


cols = pd.DataFrame.from_dict({"x": [1, 2], "y": [3, 4]})   #@row cols: present={x,y} definite={x,y} closed
wide = pd.DataFrame.from_dict({"r1": {"a": 1}, "r2": {"b": 2}}, orient="index")   #@row wide: present={a,b} definite={a,b} absent={r1,r2} closed
named = pd.DataFrame.from_dict({"r1": [1, 2]}, orient="index", columns=["p", "q"])   #@row named: present={p,q} definite={p,q} closed


def wide_c(frame):
    return frame["c"]                                    #@ missing c


if __name__ == "__main__":
    caught = []  # keeps each raised exception alive
    f1 = pd.DataFrame({"a": [1, 2], "b": [3, 4]})
    f2 = pd.DataFrame({"a": [5], "c": [6]})
    roundtrip(f1)
    try:
        roundtrip(f2)
    except KeyError as e:
        caught.append(e)
    try:
        wide_c(wide)
    except KeyError as e:
        caught.append(e)

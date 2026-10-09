# value_counts: a Series; reset_index() gives {the column's name, "count"} ("proportion" with normalize=True)
import pandas as pd

V1 = pd.DataFrame({"color": ["r", "g", "r"], "size": ["S", "S", "M"]})
V2 = pd.DataFrame({"color": ["b"], "size": ["L"], "qty": [1]})


def counts(df):
    c = df["color"].value_counts().reset_index()         #@ ok color
    #@row c: present={color,count} definite={color,count} closed
    c["count"]                                           #@ ok count
    p = df["size"].value_counts(normalize=True).reset_index()
    #@row p: present={size,proportion} definite={size,proportion} absent={count} closed
    both = df.value_counts(["color", "size"]).reset_index()
    #@row both: present={color,size,count} definite={color,size,count} absent={qty} closed
    n = df["color"].value_counts().rename_axis("val").reset_index(name="n")
    #@row n: present={val,n} definite={val,n} absent={color,count} closed
    f = df["color"].value_counts().to_frame()
    #@row f: present={count} definite={count} closed
    return c


def index_col(c):
    return c["index"]                                    #@ missing index


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for df in (V1, V2):
        try:
            index_col(counts(df))
        except KeyError as e:
            kept.append(e)

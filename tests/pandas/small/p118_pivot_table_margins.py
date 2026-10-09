# pivot_table with several values and an aggfunc dict, margins=True: the values are the columns, the margin is a row
import pandas as pd

V1 = pd.DataFrame({"k": ["a", "a", "b"], "c": ["u", "w", "u"], "x": [1, 2, 3], "y": [4.0, 5.0, 6.0], "z": [0, 0, 0]})
V2 = pd.DataFrame({"k": ["d"], "c": ["u"], "x": [7], "y": [8.0], "z": [1]})


def pivot(df):
    p = df.pivot_table(index="k", values=["x", "y"], aggfunc={"x": "sum", "y": "mean"}, margins=True)
    #@row p: present={x,y} definite={x,y} absent={k,z} closed
    tot = p.loc["All"]
    #@row tot: present={x,y} definite={x,y} closed
    flat = p.reset_index()
    #@row flat: present={k,x,y} definite={k,x,y} closed
    q = df.pivot_table(index="k", columns="c", values="x", aggfunc="sum", margins=True, margins_name="Total")
    # (first written `definite={Total} open`: the values of c are literal, so are the columns)
    #@row q: present={u,w,Total} definite={u,Total} closed
    q["Total"]                                   #@ ok Total
    return p


def zcol(p):
    return p["z"]                                #@ missing z


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for f in (V1, V2):
        try:
            zcol(pivot(f))
        except KeyError as e:
            kept.append(e)

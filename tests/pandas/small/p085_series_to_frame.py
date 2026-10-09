# Series.to_frame: one column named after the Series (or name=); an unnamed Series gives column 0
import pandas as pd


def framed(s):
    f = s.to_frame()                                     #@row f: present={qty,price} closed
    g = s.to_frame(name="value")                         #@row g: present={value} definite={value} absent={qty,price} closed
    h = s.rename("x").to_frame()                         #@row h: present={x} definite={x} closed
    g["value"]                                           #@ ok value
    return g["qty"]                                      #@ missing qty


def pair(df):
    j = df["qty"].to_frame().join(df["price"])           #@row j: present={qty,price} definite={qty,price} closed
    return j


unnamed = pd.Series([1, 2]).to_frame()                   #@row unnamed: present={0} definite={0} closed
positional = pd.Series([1, 2], name="n").to_frame("m")   #@row positional: present={m} definite={m} absent={n} closed

if __name__ == "__main__":
    caught = []  # keeps each raised exception alive
    df = pd.DataFrame({"qty": [1, 2], "price": [3.0, 4.0]})
    for s in (df["qty"], df["price"]):
        try:
            framed(s)
        except KeyError as e:
            caught.append(e)
    pair(df)
    pair(df.assign(extra=1)[["qty", "price"]])

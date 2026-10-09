# df.eval: "c = a + b" returns a frame with c added (inplace=True adds it to df); "a + b" is a Series
import pandas as pd


def with_total(df):
    out = df.eval("total = qty * price")
    #@row out: present={qty,price,total,sku} definite={qty,price,total} closed
    #@row df: present={qty,price,sku} definite={qty,price} absent={total} closed
    out["total"]                                 #@ ok total
    df["total"]                                  #@ missing total


def in_place(df):
    df.eval("net = price - disc", inplace=True)
    #@row df: present={price,disc,net,qty} definite={price,disc,net} closed
    s = df.eval("net * 2")                       # a Series, df unchanged
    #@row df: present={price,disc,net,qty} definite={price,disc,net} closed
    df["net"]                                    #@ ok net


frame = pd.DataFrame({"qty": [1, 2], "price": [3.0, 4.0]})
two = frame.eval("a = qty + 1\nb = a * 2")       #@row two: present={qty,price,a,b} definite={qty,price,a,b} closed
two["b"]                                         #@ ok b
has_a = "a" in frame                             #@value has_a: exactly={False}

if __name__ == "__main__":
    caught = []
    for df in [frame, pd.DataFrame({"qty": [5], "price": [1.0], "sku": ["s"]})]:
        try:
            with_total(df)
        except KeyError as e:
            caught.append(e)
    in_place(pd.DataFrame({"price": [3.0], "disc": [0.5]}))
    in_place(pd.DataFrame({"price": [3.0, 1.0], "disc": [0.5, 0.0], "qty": [1, 1]}))

# an alias shares column stores with its frame; copy(), df[[...]] and head() do not
import pandas as pd


def alias_vs_copy(df):
    same = df
    snap = df.copy()
    same["tax"] = 0.2
    #@row df: present={net,tax,cur} definite={net,tax} closed
    df["tax"]                                    #@ ok tax
    #@row snap: present={net,cur} definite={net} absent={tax} closed
    snap["tax"]                                  #@ missing tax


def through_param(df):
    add_gross(df)
    #@row df: present={net,gross,cur} definite={net,gross} closed
    df["gross"]                                  #@ ok gross


def add_gross(frame):
    frame["gross"] = frame["net"] * 1.2          #@ ok net


def projections(df):
    sub = df[["net"]]
    sub["flag"] = 1
    #@row sub: present={net,flag} definite={net,flag} closed
    top = df.head(1)
    top["flag2"] = 2
    #@row df: present={net,cur} definite={net} absent={flag,flag2} closed
    df["flag"]                                   #@ missing flag


base = pd.DataFrame({"net": [10.0]})
other = base
other["vat"] = 1
#@row base: present={net,vat} definite={net,vat} closed
deep = base.copy(deep=True)
deep["x"] = 0
#@row base: present={net,vat} definite={net,vat} absent={x} closed

if __name__ == "__main__":
    caught = []
    for df in [pd.DataFrame({"net": [1.0]}), pd.DataFrame({"net": [2.0, 3.0], "cur": ["eur", "usd"]})]:
        try:
            alias_vs_copy(df.copy())
        except KeyError as e:
            caught.append(e)
        through_param(df.copy())
        try:
            projections(df.copy())
        except KeyError as e:
            caught.append(e)

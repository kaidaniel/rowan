# Series.reset_index: columns = index name(s) + the Series name (name= overrides; unnamed index -> "index", unnamed Series -> 0)
import pandas as pd


def counts(df):
    vc = df["k"].value_counts().reset_index()            #@ ok k
    #@row vc: present={k,count} definite={k,count} closed
    named = df["k"].value_counts().reset_index(name="n")   #@row named: present={k,n} definite={k,n} absent={count} closed
    tot = df.groupby("k")["v"].sum().reset_index()       #@row tot: present={k,v} definite={k,v} closed
    plain = df["v"].reset_index()                        #@row plain: present={index,v} definite={index,v} closed
    keyed = df.set_index("k")["v"].reset_index()         #@row keyed: present={k,v} definite={k,v} absent={index} closed
    return vc["v"]                                       #@ missing v


unnamed = pd.Series([1, 2]).reset_index()                #@row unnamed: present={index,0} definite={index,0} closed

if __name__ == "__main__":
    caught = []  # keeps each raised exception alive
    f1 = pd.DataFrame({"k": ["x", "y", "x"], "v": [1, 2, 3]})
    f2 = pd.DataFrame({"k": ["z"], "v": [4], "w": [0]})
    for frame in (f1, f2):
        try:
            counts(frame)
        except KeyError as e:
            caught.append(e)

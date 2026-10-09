# df.columns = [...] and set_axis(..., axis=1) replace every column name
import pandas as pd


def relabel(df):
    df.columns = ["k", "v"]
    #@row df: present={k,v} definite={k,v} absent={key,val} closed
    df["v"]                                      #@ ok v
    df["key"]                                    #@ missing key


def via_set_axis(df):
    out = df.set_axis(["left", "right"], axis=1)
    #@row out: present={left,right} definite={left,right} closed
    #@row df: present={key,val,K,V} closed
    out["left"]                                  #@ ok left
    df["left"]                                   #@ missing left


frame = pd.DataFrame({"Key": [1], "Val": [2]})
frame.columns = frame.columns.str.lower()        #@row frame: present={key,val} definite={key,val} absent={Key,Val} closed
frame["key"]                                     #@ ok key
pairs = pd.DataFrame([[1, 2]]).set_axis(["a", "b"], axis="columns")  #@row pairs: present={a,b} definite={a,b} closed
n = len(pairs.columns)                           #@value n: exactly={2}

if __name__ == "__main__":
    caught = []
    for df in [pd.DataFrame({"key": [1], "val": [2]}), pd.DataFrame({"K": ["a", "b"], "V": [0, 1]})]:
        try:
            relabel(df.copy())
        except KeyError as e:
            caught.append(e)
        try:
            via_set_axis(df)
        except KeyError as e:
            caught.append(e)

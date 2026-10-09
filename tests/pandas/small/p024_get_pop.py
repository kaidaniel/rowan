# df.get(col, default) never raises; df.pop(col) removes the column (DataFrame.pop takes no default)
import pandas as pd


def take(df):
    fee = df.get("fee", 0)                       #@ sound fee
    sub = df.get(["id", "amt"])
    #@row sub: present={id,amt} definite={id,amt} closed
    amt = df.pop("amt")                          #@ ok amt
    #@row df: present={id,fee} definite={id} absent={amt} closed
    if "fee" in df:
        df.pop("fee")                            #@ ok fee
    #@row df: present={id} definite={id} absent={amt,fee} closed
    df.pop("amt")                                #@ missing amt


def defaulted(df):
    v = df.get("zz", "none")                     #@value v: exactly={"none"}
    w = df.get("zz")                             #@value w: exactly={None}
    x = df.get("id")
    #@row df: present={id,amt,fee} definite={id,amt} closed


frame = pd.DataFrame({"id": [1], "amt": [2.0]})
got = frame.get("amt")
popped = frame.pop("id")                         #@ ok id
#@row frame: present={amt} definite={amt} absent={id} closed
try:
    frame.pop("id")                              #@ missing id
except KeyError:
    pass

if __name__ == "__main__":
    caught = []
    for df in [pd.DataFrame({"id": [1], "amt": [2.0]}), pd.DataFrame({"id": [1, 2], "amt": [2.0, 3.0], "fee": [0.1, 0.2]})]:
        defaulted(df)
        try:
            take(df.copy())
        except KeyError as e:
            caught.append(e)

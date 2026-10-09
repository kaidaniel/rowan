# resample on a DatetimeIndex (or on=col): the remaining columns; agg dict / ohlc name the result columns
import pandas as pd

R1 = pd.DataFrame({"ts": pd.to_datetime(["2024-01-01", "2024-01-01", "2024-01-02"]), "v": [1, 2, 3]})
R2 = pd.DataFrame({"ts": pd.to_datetime(["2024-02-01", "2024-02-03"]), "v": [4, 5], "w": [6, 7]})


def daily(df):
    d = df.set_index("ts").resample("D").sum()
    #@row d: present={v,w} definite={v} absent={ts} closed
    m = df.resample("D", on="ts").mean()
    #@row m: present={v,w} definite={v} absent={ts} closed
    a = df.set_index("ts").resample("D").agg({"v": "max"})
    #@row a: present={v} definite={v} closed
    c = df.set_index("ts")["v"].resample("D").ohlc()
    #@row c: present={open,high,low,close} definite={open,high,low,close} closed
    back = d.reset_index()
    #@row back: present={ts,v,w} definite={ts,v} closed
    return d


def ts(d):
    return d["ts"]                                       #@ missing ts


def w(d):
    return d["w"]                                        #@ possibly w


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for df in (R1, R2):
        try:
            ts(daily(df))
        except KeyError as e:
            kept.append(e)
    w(daily(R2))
    try:
        w(daily(R1))
    except KeyError as e:
        kept.append(e)

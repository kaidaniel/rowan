# df["c"] = ... on one branch, on both branches, on an elif chain, and a drop on one branch: possibly vs ok afterwards
import pandas as pd


def flag(df, big):
    if big:
        df["flag"] = df["v"] > 10                  #@ ok v
    n = len(df.columns)                            #@row df: present={v,w,flag} definite={v} closed
    return df["flag"]                              #@ possibly flag


def label(df, mode):
    if mode == "a":
        df["label"] = "A"
    else:
        df["label"] = df["v"].astype(str)          #@ ok v
    n = len(df.columns)                            #@row df: present={v,label} definite={v,label} closed
    return df["label"]                             #@ ok label


def sign(df, x):
    if x > 0:
        df["pos"] = 1
    elif x < 0:
        df["neg"] = 1
    n = len(df.columns)                            #@row df: present={v,pos,neg} definite={v} closed
    return df["pos"]                               #@ possibly pos


def maybe_drop(df, keep):
    if not keep:
        df = df.drop(columns=["w"])
    n = len(df.columns)                            #@row df: present={v,w} definite={v} closed
    return df["w"]                                 #@ possibly w


if __name__ == "__main__":
    for f, big in ((pd.DataFrame({"v": [1, 20]}), True), (pd.DataFrame({"v": [1], "w": [2]}), False),
                   (pd.DataFrame({"v": []}), True)):
        try:
            flag(f, big)
        except KeyError:
            pass
    label(pd.DataFrame({"v": [1, 2]}), "a")
    label(pd.DataFrame({"v": [3]}), "b")
    for x in (1, -1, 0):
        try:
            sign(pd.DataFrame({"v": [x]}), x)
        except KeyError:
            pass
    for keep in (True, False):
        try:
            maybe_drop(pd.DataFrame({"v": [1], "w": [2]}), keep)
        except KeyError:
            pass

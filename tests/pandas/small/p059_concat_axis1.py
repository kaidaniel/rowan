# pd.concat(axis=1): columns side by side (Series by name); keys= makes the frames' labels the top level
import pandas as pd

LEFT = pd.DataFrame({"a": [1, 2], "b": [3, 4]})
RIGHT = pd.DataFrame({"c": [5, 6]})
RIGHT_B = pd.DataFrame({"c": [5, 6], "d": [7, 8]})


def side(l, r):
    w = pd.concat([l, r], axis=1)
    #@row w: present={a,b,c,d} definite={a,b,c} closed
    w["c"]                                               #@ ok c
    return w


def d_of(w):
    return w["d"]                                        #@ possibly d


s = pd.concat([LEFT["a"], RIGHT["c"].rename("total")], axis=1)
#@row s: present={a,total} definite={a,total} absent={c} closed
k = pd.concat([LEFT, RIGHT], axis=1, keys=["l", "r"])
k["l"]                                                   #@ ok l
k[("r", "c")]                                            #@ ok ("r", "c")


def bare(k):
    return k["a"]                                        #@ missing a


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    d_of(side(LEFT, RIGHT_B))
    try:
        d_of(side(LEFT, RIGHT))
    except KeyError as e:
        kept.append(e)
    try:
        bare(k)
    except KeyError as e:
        kept.append(e)

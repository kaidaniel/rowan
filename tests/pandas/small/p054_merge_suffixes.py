# suffixes=: custom names for overlapping non-key columns, "" keeps the left name
import pandas as pd

A = pd.DataFrame({"k": [1, 2], "v": [1.0, 2.0], "note": ["x", "y"]})
B = pd.DataFrame({"k": [1, 2], "v": [3.0, 4.0]})
B_N = pd.DataFrame({"k": [1], "v": [5.0], "note": ["z"]})


def lr(a, b):
    m = a.merge(b, on="k", suffixes=("_l", "_r"))
    #@row m: present={k,v_l,v_r,note,note_l,note_r} definite={k,v_l,v_r} absent={v,v_x,v_y} closed
    m["v_l"]                                             #@ ok v_l
    return m


def keep_left(a, b):
    m = a.merge(b, on="k", suffixes=("", "_new"))
    #@row m: present={k,v,v_new,note,note_new} definite={k,v,v_new,note} closed
    m["v"]                                               #@ ok v
    m["v_new"]                                           #@ ok v_new
    return m


def note_r(m):
    return m["note_r"]                                   #@ possibly note_r


def v_plain(m):
    return m["v"]                                        #@ missing v


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for b in (B, B_N):
        keep_left(A, b)
    note_r(lr(A, B_N))
    try:
        note_r(lr(A, B))
    except KeyError as e:
        kept.append(e)
    try:
        v_plain(lr(A, B))
    except KeyError as e:
        kept.append(e)

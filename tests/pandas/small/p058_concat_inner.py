# pd.concat(join="inner"): only the columns every frame has
import pandas as pd

A = pd.DataFrame({"id": [1], "x": [1.0], "y": [2.0]})
B = pd.DataFrame({"id": [2], "x": [3.0]})
C = pd.DataFrame({"id": [3], "x": [4.0], "y": [5.0], "z": [6.0]})


def common(a, b):
    s = pd.concat([a, b], join="inner", ignore_index=True)
    #@row s: present={id,x,y} definite={id,x} absent={z} closed
    s["x"]                                               #@ ok x
    return s


def y_of(s):
    return s["y"]                                        #@ possibly y


ab = pd.concat([A, B], join="inner")
#@row ab: present={id,x} definite={id,x} absent={y} closed


def z_of(s):
    return s["z"]                                        #@ missing z


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    y_of(common(A, C))
    try:
        y_of(common(A, B))
    except KeyError as e:
        kept.append(e)
    for pair in ((A, B), (A, C), (B, C)):
        try:
            z_of(common(*pair))
        except KeyError as e:
            kept.append(e)

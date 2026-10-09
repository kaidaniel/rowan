# how="cross": every column of both sides, no key; overlapping columns get _x / _y
import pandas as pd

SIZES = pd.DataFrame({"size": ["S", "M"], "code": [1, 2]})
COLORS = pd.DataFrame({"color": ["red", "blue"]})
COLORS_C = pd.DataFrame({"color": ["red"], "code": [9]})


def grid(a, b):
    g = a.merge(b, how="cross")
    #@row g: present={size,code,color,code_x,code_y} definite={size,color} closed
    g["size"]                                            #@ ok size
    g["color"]                                           #@ ok color
    return g


def code(g):
    return g["code"]                                     #@ possibly code


def code_x(g):
    return g["code_x"]                                   #@ possibly code_x


c = pd.merge(SIZES, COLORS_C, how="cross")
#@row c: present={size,code_x,color,code_y} definite={size,code_x,color,code_y} absent={code} closed

kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    g1 = grid(SIZES, COLORS)
    g2 = grid(SIZES, COLORS_C)
    code(g1)
    code_x(g2)
    try:
        code(g2)
    except KeyError as e:
        kept.append(e)
    try:
        code_x(g1)
    except KeyError as e:
        kept.append(e)

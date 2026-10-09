# pd.concat(axis=1, join="inner") mixing frames and Series: join= only trims rows, the columns are the union (a named Series by its name, an unnamed one by position); along rows join="inner" keeps the shared columns
import pandas as pd


def side(a, s):
    c = pd.concat([a, s], axis=1, join="inner")
    #@row c: present={k,v,w,score} definite={k,v,score} closed
    c["score"]                                   #@ ok score
    u = pd.concat([a, s.rename(None)], axis=1, join="inner")
    #@row u: present={k,v,w,0} definite={k,v,0} absent={score} closed
    u[0]                                         #@ ok 0
    r = pd.concat([a, pd.DataFrame({"k": [9], "z": [1]})], join="inner")
    #@row r: present={k} definite={k} absent={v,w,z} closed
    return r


def read_v(r):
    return r["v"]                                #@ missing v


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for a in (pd.DataFrame({"k": [1, 2], "v": [3, 4]}),
              pd.DataFrame({"k": [5], "v": [6], "w": [7]})):
        s = pd.Series([0.5, 0.25, 0.1], name="score")
        try:
            read_v(side(a, s))
        except KeyError as e:
            kept.append(e)

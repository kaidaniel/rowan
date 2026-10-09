# set_index([a, b]) then reset_index(level=…) brings back only that level; names= renames what comes back
import pandas as pd

R1 = pd.DataFrame({"a": [1, 2], "b": ["x", "y"], "v": [3.0, 4.0]})
R2 = pd.DataFrame({"a": [5], "b": ["z"], "v": [6.0], "w": [7]})


def partial(df):
    ix = df.set_index(["a", "b"])
    #@row ix: present={v,w} definite={v} absent={a,b} closed
    one = ix.reset_index(level="b")
    #@row one: present={b,v,w} definite={b,v} absent={a} closed
    gone = ix.reset_index(level=0, drop=True)
    #@row gone: present={v,w} definite={v} absent={a,b} closed
    both = ix.reset_index(names=["p", "q"])
    #@row both: present={p,q,v,w} definite={p,q,v} absent={a,b} closed
    named = df.set_index("a").reset_index(names="key")
    #@row named: present={key,b,v,w} definite={key,b,v} absent={a} closed
    one["b"]                                     #@ ok b
    return one


def read_a(one):
    return one["a"]                              #@ missing a


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for f in (R1, R2):
        try:
            read_a(partial(f))
        except KeyError as e:
            kept.append(e)

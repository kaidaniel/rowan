# Frames from Series: dict of Series -> its keys; list of Series -> union of their indexes; concat(axis=1) -> their names
import pandas as pd


def from_series(s1, s2):
    f = pd.DataFrame({"left": s1, "right": s2})          #@row f: present={left,right} definite={left,right} closed
    rows = pd.DataFrame([s1, s2])                        #@row rows: present={a,b,c} definite={a,b} closed
    side = pd.concat([s1, s2], axis=1)                   #@row side: present={x,y,z} definite={x} closed
    keyed = pd.concat({"p": s1, "q": s2}, axis=1)        #@row keyed: present={p,q} definite={p,q} absent={x} closed
    keyed["p"]                                           #@ ok p
    return side["y"]                                     #@ ok y ; missing y


nested = pd.DataFrame({"r": {"a": 1}, "s": {"b": 2}})    #@row nested: present={r,s} definite={r,s} absent={a,b} closed
unnamed = pd.concat([pd.Series([1]), pd.Series([2])], axis=1)   #@row unnamed: present={0,1} definite={0,1} closed

if __name__ == "__main__":
    caught = []  # keeps each raised exception alive
    from_series(pd.Series({"a": 1, "b": 2}, name="x"), pd.Series({"a": 3, "c": 4}, name="y"))
    try:
        from_series(pd.Series({"a": 5}, name="x"), pd.Series({"b": 6}, name="z"))
    except KeyError as e:
        caught.append(e)

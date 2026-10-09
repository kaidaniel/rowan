# itertuples: namedtuple rows (Index + columns), index=False drops Index, name=None gives plain tuples
import pandas as pd


def first_qty(df):
    for t in df.itertuples():
        q = t.qty                                        #@ ok qty
        #@row t: present={Index,qty,price,region} definite={Index,qty} closed
        return q
    return None


def no_index(df):
    for t in df.itertuples(index=False):
        q = t.qty                                        #@ ok qty
        #@row t: present={qty,price,region} definite={qty} absent={Index} closed
        return t.Index                                   #@ missing Index
    return None


def region_of(df):
    for t in df.itertuples(index=False):
        return t.region                                  #@ ok region ; missing region
    return None


def plain(df):
    for t in df.itertuples(index=False, name=None):
        n = len(t)                                       #@value n: exactly={2,3}
        return t.qty                                     #@ missing qty
    return None


odd = pd.DataFrame({"qty": [1], "unit price": [2.5]})
for t in odd.itertuples():
    p = t._2                                             #@ ok _2
    #@row t: present={Index,qty,_2} definite={Index,qty,_2} absent={"unit price"} closed

if __name__ == "__main__":
    caught = []  # keeps each raised exception alive
    a = pd.DataFrame({"qty": [1, 2], "price": [3.0, 4.0]})
    b = pd.DataFrame({"qty": [7], "region": ["n"]})
    c = pd.DataFrame({"qty": [1], "price": [1.0], "region": ["s"]})
    for f in (first_qty, no_index, plain):
        for frame in (a, b, c):
            try:
                f(frame)
            except AttributeError as e:
                caught.append(e)
    region_of(b)
    try:
        region_of(a)
    except AttributeError as e:
        caught.append(e)

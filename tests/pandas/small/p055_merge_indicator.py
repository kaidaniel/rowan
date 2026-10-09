# indicator=True adds _merge (a string names the column); dropping it afterwards removes it
import pandas as pd

OLD = pd.DataFrame({"sku": ["a", "b"], "price": [1.0, 2.0]})
NEW = pd.DataFrame({"sku": ["b", "c"], "price": [2.5, 3.0]})
NEW_Q = pd.DataFrame({"sku": ["b"], "qty": [4]})


def diff(old, new):
    m = old.merge(new, on="sku", how="outer", indicator=True)
    #@row m: present={sku,_merge,price,price_x,price_y,qty} definite={sku,_merge} closed
    m["_merge"]                                          #@ ok _merge
    gone = m[m["_merge"] == "left_only"].drop(columns="_merge")
    #@row gone: present={sku,price,price_x,price_y,qty} definite={sku} absent={_merge} closed
    return gone


def named(old, new):
    m = pd.merge(old, new, on="sku", how="left", indicator="src")
    #@row m: present={sku,src,price,price_x,price_y,qty} definite={sku,src} absent={_merge} closed
    m["src"]                                             #@ ok src
    return m


def merge_col(m):
    return m["_merge"]                                   #@ missing _merge


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for new in (NEW, NEW_Q):
        diff(OLD, new)
        try:
            merge_col(named(OLD, new))
        except KeyError as e:
            kept.append(e)

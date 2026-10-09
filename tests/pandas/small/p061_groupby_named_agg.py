# groupby().agg(name=(col, func)): closed {names}, the key is the index until reset_index / as_index=False
import pandas as pd

S1 = pd.DataFrame({"region": ["e", "w", "e"], "amount": [1.0, 2.0, 3.0], "order_id": [1, 2, 3]})
S2 = pd.DataFrame({"region": ["n"], "amount": [4.0], "order_id": [9], "rep": ["z"]})


def totals(df):
    g = df.groupby("region").agg(total=("amount", "sum"), n=("order_id", "count"))   #@ ok region
    #@row g: present={total,n} definite={total,n} closed
    g["total"]                                           #@ ok total
    r = g.reset_index()
    #@row r: present={region,total,n} definite={region,total,n} closed
    flat = df.groupby("region", as_index=False).agg(top=("amount", "max"))
    #@row flat: present={region,top} definite={region,top} closed
    return g


def amount(g):
    return g["amount"]                                   #@ missing amount


def region(g):
    return g["region"]                                   #@ missing region


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for df in (S1, S2):
        g = totals(df)
        for f in (amount, region):
            try:
                f(g)
            except KeyError as e:
                kept.append(e)

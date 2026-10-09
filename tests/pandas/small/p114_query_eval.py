# query with a backticked name and an @local; eval with several assignments (a copy) and inplace=True (the receiver)
import pandas as pd

TWO = """
total = `unit price` * qty
big = total > 10
"""


def filt(df, lo):
    limit = lo
    q = df.query("`unit price` > @limit and qty > 0")
    #@row q: present={"unit price",qty,name,extra} definite={"unit price",qty,name} closed
    e = df.eval(TWO)
    #@row e: present={"unit price",qty,name,extra,total,big} definite={"unit price",qty,name,total,big} closed
    e["big"]                                     #@ ok big
    res = df.eval("double = qty * 2", inplace=True)   #@value res: exactly={None}
    #@row df: present={"unit price",qty,name,extra,double} definite={"unit price",qty,name,double} closed
    return df


def total(df):
    return df["total"]                           #@ missing total


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for f, lo in ((pd.DataFrame({"unit price": [2.0, 9.0], "qty": [3, 0], "name": ["a", "b"]}), 1.0),
                  (pd.DataFrame({"unit price": [5.0], "qty": [4], "name": ["c"], "extra": [1]}), 10.0)):
        try:
            total(filt(f, lo))
        except KeyError as e:
            kept.append(e)

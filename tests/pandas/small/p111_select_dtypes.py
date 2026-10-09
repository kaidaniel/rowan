# select_dtypes(include="number"): after astype, after a merge, and with bool columns from notna() (bool is not a number)
import pandas as pd

PRICES = pd.DataFrame({"code": [7, 8], "price": [1.5, 2.5], "label": ["p", "q"]})


def numeric(df):
    df = df.astype({"code": "int64"})
    num = df.select_dtypes(include="number")
    #@row num: present={qty,code} definite={qty,code} absent={name} closed
    m = df.merge(PRICES, on="code")
    mnum = m.select_dtypes(include="number")
    #@row mnum: present={qty,code,price} definite={qty,code,price} absent={name,label} closed
    df["ok"] = df["qty"].notna()
    nb = df.select_dtypes(include="number")
    #@row nb: present={qty,code} definite={qty,code} absent={ok,name} closed
    rest = df.select_dtypes(exclude="number")
    #@row rest: present={name,ok} definite={name,ok} absent={qty,code} closed
    flags = df.notna().select_dtypes(include="bool")
    #@row flags: present={name,qty,code,ok} definite={name,qty,code,ok} closed
    return nb


def read_name(nb):
    return nb["name"]                            #@ missing name


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for f in (pd.DataFrame({"name": ["a", "b"], "qty": [1, 2], "code": ["7", "9"]}),
              pd.DataFrame({"name": ["c"], "qty": [3.5], "code": ["8"]})):
        try:
            read_name(numeric(f))
        except KeyError as e:
            kept.append(e)

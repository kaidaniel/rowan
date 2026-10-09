# rename(columns=str.upper) / rename(columns=lambda c: …) on a closed frame and on an open read_csv frame
import io

import pandas as pd

CSV = "qty,price\n1,2.0\n3,4.0\n"


def up(df):
    u = df.rename(columns=str.upper)
    #@row u: present={ID,NAME,NOTE} definite={ID,NAME} absent={id,name,note} closed
    s = df.rename(columns=lambda c: c + "_v")
    #@row s: present={id_v,name_v,note_v} definite={id_v,name_v} absent={id,name} closed
    u["NAME"]                                    #@ ok NAME
    return s


def old(s):
    return s["id"]                               #@ missing id


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

raw = pd.read_csv(io.StringIO(CSV))
ru = raw.rename(columns=str.upper)
ru["QTY"]                                        #@ input QTY
#@row ru: definite={QTY} absent={qty} open
try:
    ru["qty"]                                    #@ missing qty
except KeyError as e:
    kept.append(e)
# (QTY can come from qty, Qty or QTY in the file: no one name is required)
#@schema read_csv@L24: only={qty}

if __name__ == "__main__":
    for f in (pd.DataFrame({"id": [1], "name": ["a"]}), pd.DataFrame({"id": [2], "name": ["b"], "note": ["n"]})):
        try:
            old(up(f))
        except KeyError as e:
            kept.append(e)

# pivot(index=, columns=, values=): the columns become the values of the columns= column (here literal data); without values= the top level is the value columns
import io

import pandas as pd

CSV = "day,kind,amount,memo\nmon,in,3,x\nmon,out,1,y\ntue,in,5,z\n"


def wide(df):
    w = df.pivot(index="day", columns="kind", values="amount")
    #@row w: present={in,out,fee} definite={in,out} absent={day,kind,amount} closed
    w["in"]                                        #@ ok in
    flat = w.reset_index()
    #@row flat: present={day,in,out,fee} definite={day,in,out} absent={kind,amount} closed
    flat["day"]                                    #@ ok day
    w["day"]                                       #@ missing day


def both_values(df):
    w = df.pivot(index="day", columns="kind")
    w["amount"]                                    #@ ok amount
    w["in"]                                        #@ missing in


raw = pd.read_csv(io.StringIO(CSV))
pr = raw.pivot(index="day", columns="kind", values="amount")
pr["in"]                                           #@ input in
rp = pr.reset_index()
# (first written definite={day}: the access pr["in"] above went through, so "in" is there)
#@row rp: definite={day,in} open
#@schema read_csv@L25: always={day,kind,amount} only={day,kind,amount}

if __name__ == "__main__":
    a = pd.DataFrame({"day": ["mon", "mon", "tue"], "kind": ["in", "out", "in"], "amount": [3, 1, 5], "qty": [1, 1, 2]})
    b = pd.DataFrame({"day": ["mon", "mon", "mon"], "kind": ["in", "out", "fee"], "amount": [2, 4, 1], "qty": [3, 1, 1]})
    for f in (a, b):
        for fn in (wide, both_values):
            try:
                fn(f)
            except KeyError:
                pass

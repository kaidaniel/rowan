# pd.concat along rows: the union of the columns (NaN-filled), ignore_index / keys= leave the columns alone
import pandas as pd

JAN = pd.DataFrame({"day": [1, 2], "sales": [10, 20]})
FEB = pd.DataFrame({"day": [1], "sales": [5], "promo": [True]})
MAR = pd.DataFrame({"day": [3], "returns": [1]})


def stack(a, b):
    s = pd.concat([a, b], ignore_index=True)
    #@row s: present={day,sales,promo,returns} definite={day} closed
    s["day"]                                             #@ ok day
    return s


def promo(s):
    return s["promo"]                                    #@ possibly promo


both = pd.concat([JAN, FEB])
#@row both: present={day,sales,promo} definite={day,sales,promo} closed
keyed = pd.concat([JAN, MAR], keys=["jan", "mar"])
#@row keyed: present={day,sales,returns} definite={day,sales,returns} closed
keyed["returns"]                                         #@ ok returns
tagged = pd.concat([JAN.assign(month=1), FEB.assign(month=2)])
#@row tagged: present={day,sales,promo,month} definite={day,sales,promo,month} closed


def month_gone(t):
    return t.drop(columns=["month"])["month"]            #@ missing month


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    promo(stack(JAN, FEB))
    try:
        promo(stack(JAN, MAR))
    except KeyError as e:
        kept.append(e)
    stack(FEB, MAR)
    try:
        month_gone(tagged)
    except KeyError as e:
        kept.append(e)

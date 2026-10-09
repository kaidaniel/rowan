# pd.cut with labels=[…]: value_counts() of the categorical has every label (zero counts too), so to_dict() keys are the labels
import pandas as pd

P1 = pd.DataFrame({"age": [5, 30, 70, 40], "sex": ["f", "m", "f", "m"]})
P2 = pd.DataFrame({"age": [30], "sex": ["m"]})


def bands(df):
    band = pd.cut(df["age"], bins=[0, 18, 65, 120], labels=["kid", "adult", "senior"])
    counts = band.value_counts().to_dict()
    #@row counts: present={kid,adult,senior} definite={kid,adult,senior} closed
    n = counts["senior"]                         #@ ok senior
    tab = pd.crosstab(df["sex"], band, dropna=False)
    #@row tab: present={kid,adult,senior} definite={kid,adult,senior} closed
    tab["kid"]                                   #@ ok kid
    return counts


def child(counts):
    return counts["child"]                       #@ missing child


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for f in (P1, P2):
        try:
            child(bands(f))
        except KeyError as e:
            kept.append(e)

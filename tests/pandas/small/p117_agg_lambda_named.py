# groupby agg with a lambda and a named function: the columns are "<lambda_0>", "<lambda_1>", … and the function's name
import pandas as pd

G1 = pd.DataFrame({"k": ["a", "a", "b"], "x": [1, 5, 2]})
G2 = pd.DataFrame({"k": ["c"], "x": [3], "y": [4]})


def spread(s):
    return s.max() - s.min()


def summary(df):
    g = df.groupby("k")["x"].agg(["sum", spread, lambda s: s.iloc[0]])
    # (first written "<lambda>": in a list pandas numbers even a single lambda, "<lambda_0>")
    #@row g: present={sum,spread,"<lambda_0>"} definite={sum,spread,"<lambda_0>"} absent={"<lambda>"} closed
    g["spread"]                                  #@ ok spread
    two = df.groupby("k")["x"].agg([lambda s: s.min(), lambda s: s.max()])
    #@row two: present={"<lambda_0>","<lambda_1>"} definite={"<lambda_0>","<lambda_1>"} closed
    named = df.groupby("k", as_index=False).agg(rng=("x", spread))
    #@row named: present={k,rng} definite={k,rng} closed
    return g


def by_col(g):
    return g["x"]                                #@ missing x


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for f in (G1, G2):
        try:
            by_col(summary(f))
        except KeyError as e:
            kept.append(e)

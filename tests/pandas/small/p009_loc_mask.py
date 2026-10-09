# df.loc[mask, "col"] reads, df.loc[mask, "new"] = v adds a column, df.loc[mask] keeps columns
import pandas as pd


def flag_big(df):
    big = df.loc[df["amount"] > 10]              #@ ok amount
    #@row big: present={amount,cust,region} definite={amount,cust} closed
    names = df.loc[df["amount"] > 10, "cust"]    #@ ok amount ; ok cust
    df.loc[df["amount"] > 10, "big"] = True
    #@row df: present={amount,cust,region,big} definite={amount,cust,big} closed
    df["big"]                                    #@ ok big
    df.loc[df["amount"] > 0, "region"]           #@ ok region ; missing region


def two_cols(df):
    part = df.loc[df["cust"] != "", ["cust", "amount"]]
    #@row part: present={cust,amount} definite={cust,amount} absent={region} closed
    part["amount"]                               #@ ok amount
    part["region"]                               #@ missing region


orders = pd.DataFrame({"cust": ["a", "b"], "amount": [5, 20]})
orders.loc[orders["amount"] < 0, "neg"] = 1      # no row matches: the column is still added
#@row orders: present={cust,amount,neg} definite={cust,amount,neg} closed
orders["neg"]                                    #@ ok neg

if __name__ == "__main__":
    caught = []
    for df in [pd.DataFrame({"cust": ["a", "b"], "amount": [5, 20], "region": ["n", "s"]}), pd.DataFrame({"cust": ["c"], "amount": [1]})]:
        try:
            flag_big(df.copy())
        except KeyError as e:
            caught.append(e)
        try:
            two_cols(df)
        except KeyError as e:
            caught.append(e)

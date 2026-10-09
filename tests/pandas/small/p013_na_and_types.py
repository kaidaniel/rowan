# dropna(subset=) / fillna(dict) / astype(dict) keep the columns; dropna(axis=1) drops data-chosen ones
import pandas as pd


def clean(df):
    d = df.dropna(subset=["qty"])                # subset= names a column that must exist
    #@row d: present={qty,price,note} definite={qty,price} closed
    f = df.fillna({"price": 0.0, "zz": 1})       # unknown keys in the dict are ignored
    #@row f: present={qty,price,note} definite={qty,price} absent={zz} closed
    a = df.astype({"qty": "float64"})
    #@row a: present={qty,price,note} definite={qty,price} closed
    a["qty"]                                     #@ ok qty
    cols = df.dropna(axis=1)                     # drops the columns that hold a NaN
    #@row cols: present={qty,price,note} closed
    cols["price"]                                #@ possibly price


frame = pd.DataFrame({"qty": [1, None], "price": [2.0, 3.0]})
filled = frame.fillna(0).astype({"qty": int})    #@row filled: present={qty,price} definite={qty,price} closed
filled["qty"]                                    #@ ok qty
kept = frame.dropna(axis=1, how="all")           #@row kept: present={qty,price} closed
nn = frame.dropna()                              #@row nn: present={qty,price} definite={qty,price} closed
nn["price"]                                      #@ ok price

if __name__ == "__main__":
    caught = []
    for df in [frame, pd.DataFrame({"qty": [1.0], "price": [None], "note": ["x"]}), pd.DataFrame({"qty": [2.0, 3.0], "price": [1.0, 1.5]})]:
        try:
            clean(df)
        except KeyError as e:
            caught.append(e)

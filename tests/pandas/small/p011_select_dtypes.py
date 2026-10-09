# select_dtypes(include= / exclude=): columns chosen by dtype, derivable when the dtypes are literal
import pandas as pd


def numeric_part(df):
    num = df.select_dtypes(include="number")
    #@row num: present={qty,price} definite={qty} absent={name,ok} closed
    num["qty"]                                   #@ ok qty
    rest = df.select_dtypes(exclude="number")
    #@row rest: present={name,ok} absent={qty,price} closed
    num["name"]                                  #@ missing name


def floats_only(df):
    fl = df.select_dtypes(include=["float"])
    #@row fl: present={price} absent={qty,name,ok} closed
    fl["qty"]                                    #@ missing qty


frame = pd.DataFrame({"name": ["a", "b"], "qty": [1, 2], "price": [1.5, 2.5], "ok": [True, False]})
n = frame.select_dtypes(include="number")        #@row n: present={qty,price} definite={qty,price} closed
b = frame.select_dtypes(include="bool")          #@row b: present={ok} definite={ok} closed
ints = frame.select_dtypes(include="int64")      #@row ints: present={qty} definite={qty} closed
ints["qty"]                                      #@ ok qty

if __name__ == "__main__":
    caught = []
    for df in [frame, pd.DataFrame({"qty": [3], "name": ["z"]}), pd.DataFrame({"qty": [1, 2], "price": [0.5, 1.0]})]:
        try:
            numeric_part(df)
        except KeyError as e:
            caught.append(e)
        try:
            floats_only(df)
        except KeyError as e:
            caught.append(e)

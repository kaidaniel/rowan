# df.col vs df["col"]: attribute reads see columns, attribute stores do not create one
import warnings

import pandas as pd

warnings.simplefilter("ignore")


def read_attr(df):
    s = df.price                                 #@ ok price
    t = df["price"]                              #@ ok price
    df.qty                                       #@ ok qty ; missing qty


def store_attr(df):
    df.total = df["price"] * 2                   #@ ok price
    #@row df: present={price,qty} definite={price} absent={total} closed
    df["total"]                                  #@ missing total


def shadowed(df):
    c = df["count"]                              #@ ok count
    m = df.count                                 # the method, not the column
    #@row df: present={count,size} definite={count} closed
    df["size"]                                   #@ ok size ; missing size


frame = pd.DataFrame({"price": [1.0, 2.0], "qty": [1, 3]})
frame.price                                      #@ ok price
try:
    frame.weight                                 #@ missing weight
except AttributeError:
    pass

if __name__ == "__main__":
    caught = []
    for df in [pd.DataFrame({"price": [1.0], "qty": [2]}), pd.DataFrame({"price": [3.0, 4.0]})]:
        try:
            read_attr(df)
        except AttributeError as e:
            caught.append(e)
        try:
            store_attr(df.copy())
        except KeyError as e:
            caught.append(e)
    for df in [pd.DataFrame({"count": [1], "size": [2]}), pd.DataFrame({"count": [5, 6]})]:
        try:
            shadowed(df)
        except KeyError as e:
            caught.append(e)

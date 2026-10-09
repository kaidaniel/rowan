# merge on one key column: the key once, both sides' columns, every how= keeps the same columns
import pandas as pd

LEFT = pd.DataFrame({"id": [1, 2, 3], "name": ["a", "b", "c"]})
R1 = pd.DataFrame({"id": [1, 2], "qty": [5, 6]})
R2 = pd.DataFrame({"id": [2, 4], "qty": [7, 8], "price": [1.0, 2.0]})


def combine(left, right, how):
    m = left.merge(right, on="id", how=how)              #@ ok id
    #@row m: present={id,name,qty,price} definite={id,name,qty} closed
    m["name"]                                            #@ ok name
    return m


def price_of(m):
    return m["price"]                                    #@ possibly price


def no_suffix(m):
    return m["id_x"]                                     #@ missing id_x


m1 = pd.merge(LEFT, R1, on="id")
#@row m1: present={id,name,qty} definite={id,name,qty} closed
m2 = LEFT.merge(R2, on="id", how="outer")
#@row m2: present={id,name,qty,price} definite={id,name,qty,price} closed

kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for how in ["inner", "left", "right", "outer"]:
        combine(LEFT, R1, how)
        combine(LEFT, R2, how)
    price_of(combine(LEFT, R2, "left"))
    try:
        price_of(combine(LEFT, R1, "inner"))
    except KeyError as e:
        kept.append(e)
    try:
        no_suffix(m2)
    except KeyError as e:
        kept.append(e)

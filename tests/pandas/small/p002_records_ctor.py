# DataFrame from a list of record dicts and DataFrame.from_records (columns= / exclude=)
import pandas as pd


def from_dicts(recs):
    df = pd.DataFrame(recs)
    #@row df: present={sku,qty,price,note} definite={sku,qty} closed
    df["qty"]                                    #@ ok qty
    df["price"]                                  #@ ok price ; missing price


def records_selected(recs):
    df = pd.DataFrame.from_records(recs, columns=["sku", "qty"])
    #@row df: present={sku,qty} definite={sku,qty} absent={price} closed
    df["sku"]                                    #@ ok sku
    df["price"]                                  #@ missing price


def records_excluded(rows):
    df = pd.DataFrame.from_records(rows, columns=["sku", "qty", "price"], exclude=["price"])
    #@row df: present={sku,qty} definite={sku,qty} absent={price} closed
    df["qty"]                                    #@ ok qty
    df["price"]                                  #@ missing price


mixed = pd.DataFrame([{"a": 1}, {"b": 2}, {"a": 3, "c": 4}])
#@row mixed: present={a,b,c} definite={a,b,c} closed
mixed["b"]                                       #@ ok b
fr = pd.DataFrame.from_records([{"x": 1, "y": 2}, {"x": 3}])
#@row fr: present={x,y} definite={x,y} closed
fr["y"]                                          #@ ok y

if __name__ == "__main__":
    caught = []
    for recs in [[{"sku": "a", "qty": 1, "price": 2.0}], [{"sku": "b", "qty": 2}, {"sku": "c", "qty": 0, "note": "x"}]]:
        try:
            from_dicts(recs)
        except KeyError as e:
            caught.append(e)
        try:
            records_selected(recs)
        except KeyError as e:
            caught.append(e)
    for rows in [[("a", 1, 2.0)], [("b", 2, 3.5), ("c", 0, 1.0)]]:
        try:
            records_excluded(rows)
        except KeyError as e:
            caught.append(e)

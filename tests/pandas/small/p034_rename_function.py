# rename with a function: columns=str.upper / a def, mapper + axis=1 / axis="columns", the default axis renames the index only, set_axis on the columns
import pandas as pd


def snake_name(c):
    return c.strip().lower().replace(" ", "_")


def upper(df):
    out = df.rename(columns=str.upper)
    #@row out: present={A,B} definite={A} absent={a,b} closed
    out["A"]                                       #@ ok A
    out["a"]                                       #@ missing a


def stripped(df):
    out = df.rename(str.strip, axis=1)
    #@row out: present={a,b} definite={a} absent={" a","b "} closed
    return out["a"]                                #@ ok a


def mapper_axis(df):
    out = df.rename({"a": "x"}, axis="columns")
    #@row out: present={x,b} definite={x,b} absent={a} closed
    idx = df.rename({"a": "x"})
    #@row idx: present={a,b} definite={a,b} absent={x} closed
    return idx["a"]                                #@ ok a


def snake(df):
    out = df.rename(columns=snake_name)
    #@row out: present={order_id,unit_price} definite={order_id} absent={"Order ID","Unit Price"} closed
    return out["order_id"]                         #@ ok order_id


def relabel(df):
    out = df.set_axis(["p", "q"], axis=1)
    #@row out: present={p,q} definite={p,q} absent={a,b} closed
    return out


if __name__ == "__main__":
    for f in (pd.DataFrame({"a": [1]}), pd.DataFrame({"a": [1], "b": [2]})):
        try:
            upper(f)
        except KeyError:
            pass
    stripped(pd.DataFrame({" a": [1], "b ": [2]}))
    stripped(pd.DataFrame({" a": [1]}))
    mapper_axis(pd.DataFrame({"a": [1], "b": [2]}))
    mapper_axis(pd.DataFrame({"a": [], "b": []}))
    snake(pd.DataFrame({"Order ID": [1], "Unit Price": [2.0]}))
    snake(pd.DataFrame({"Order ID": [1]}))
    snake(pd.DataFrame({" Order ID ": [1]}))
    relabel(pd.DataFrame({"a": [1], "b": [2]}))
    relabel(pd.DataFrame({"a": [], "b": []}))

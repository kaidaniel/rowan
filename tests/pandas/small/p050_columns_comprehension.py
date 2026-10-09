# columns renamed by comprehensions: a list comprehension over df.columns, a conditional one, a dict comprehension for rename, dict(zip(columns, names))
import pandas as pd


def suffixed(df):
    df.columns = [c + "_raw" for c in df.columns]
    #@row df: present={a_raw,b_raw} definite={a_raw} absent={a,b} closed
    df["a_raw"]                                    #@ ok a_raw
    df["a"]                                        #@ missing a


def upper_but_id(df):
    out = df.rename(columns={c: c.upper() for c in df.columns if c != "id"})
    #@row out: present={id,X,Y} definite={id,X} absent={x,y,ID} closed
    return out["id"]                               #@ ok id


def tagged(df):
    df.columns = [f"n_{c}" if c.startswith("v") else c for c in df.columns]
    #@row df: present={id,n_v1,n_v2} definite={id,n_v1} absent={v1,v2,n_id} closed
    return df["n_v1"]                              #@ ok n_v1


def zipped(df, names):
    out = df.rename(columns=dict(zip(df.columns, names)))
    #@row out: present={p,q} definite={p,q} absent={a,b} closed
    return out


frame = pd.DataFrame({"First Name": ["a"], "Last Name": ["b"]})
frame.columns = [c.lower().replace(" ", "_") for c in frame.columns]
#@row frame: present={first_name,last_name} definite={first_name,last_name} absent={"First Name","Last Name"} closed
frame["first_name"]                                #@ ok first_name

if __name__ == "__main__":
    for f in (pd.DataFrame({"a": [1]}), pd.DataFrame({"a": [1], "b": [2]})):
        try:
            suffixed(f)
        except KeyError:
            pass
    upper_but_id(pd.DataFrame({"id": [1], "x": [2]}))
    upper_but_id(pd.DataFrame({"id": [1], "x": [2], "y": [3]}))
    tagged(pd.DataFrame({"id": [1], "v1": [2]}))
    tagged(pd.DataFrame({"id": [1], "v1": [2], "v2": [3]}))
    zipped(pd.DataFrame({"a": [1], "b": [2]}), ["p", "q"])
    zipped(pd.DataFrame({"a": [], "b": []}), ("p", "q"))

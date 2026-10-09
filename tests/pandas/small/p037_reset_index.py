# reset_index: an unnamed index comes back as "index", a named one under its name, drop=True adds nothing, names=, level= on a MultiIndex, after groupby / rename_axis
import pandas as pd


def unnamed(df):
    out = df.reset_index()
    #@row out: present={index,a,b} definite={index,a} closed
    return out["index"]                            #@ ok index


def dropped(df):
    out = df.reset_index(drop=True)
    #@row out: present={a,b} definite={a} absent={index} closed
    out["index"]                                   #@ missing index


def named(df):
    out = df.set_index("k").reset_index(names="key")
    #@row out: present={key,a} definite={key,a} absent={k,index} closed
    return out


def one_level(df):
    m = df.set_index(["k", "j"])
    out = m.reset_index(level="j")
    #@row out: present={j,a} definite={j,a} absent={k} closed
    out2 = m.reset_index(level=0)
    #@row out2: present={k,a} definite={k,a} absent={j} closed
    return out2


def grouped(df):
    out = df.groupby("k")["a"].sum().reset_index()
    #@row out: present={k,a} definite={k,a} closed
    sizes = df.groupby("k").size().reset_index(name="n")
    #@row sizes: present={k,n} definite={k,n} closed
    return sizes


def axis_named(df):
    out = df.rename_axis("row").reset_index()
    #@row out: present={row,a,k,j} definite={row,a} absent={index} closed
    return out


if __name__ == "__main__":
    unnamed(pd.DataFrame({"a": [1]}))
    unnamed(pd.DataFrame({"a": [1, 2], "b": [3, 4]}))
    for f in (pd.DataFrame({"a": [1]}), pd.DataFrame({"a": [1], "b": [2]})):
        try:
            dropped(f)
        except KeyError:
            pass
    for f in (pd.DataFrame({"k": ["x", "y"], "j": [1, 2], "a": [3, 4]}), pd.DataFrame({"k": [], "j": [], "a": []})):
        one_level(f)
        grouped(f)
        axis_named(f)
        named(f.drop(columns=["j"]))
    axis_named(pd.DataFrame({"a": [1]}))

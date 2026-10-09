# add_prefix / add_suffix rename every column (axis=0 renames the index instead); prefixing both sides before concat(axis=1)
import io

import pandas as pd

CSV = "x,y\n1,2\n"


def pre(df):
    out = df.add_prefix("l_")
    #@row out: present={l_a,l_b} definite={l_a} absent={a,b} closed
    out["l_a"]                                     #@ ok l_a
    out["a"]                                       #@ missing a


def suf(df):
    out = df.add_suffix("_2024")
    #@row out: present={a_2024,b_2024} definite={a_2024,b_2024} absent={a,b} closed
    return out


def pre_index(df):
    out = df.add_prefix("r", axis=0)
    #@row out: present={a,b} definite={a,b} absent={ra,rb} closed
    return out["a"]                                #@ ok a


def side_by_side(left, right):
    both = pd.concat([left.add_prefix("l_"), right.add_prefix("r_")], axis=1)
    #@row both: present={l_k,l_v,r_k,r_w} definite={l_k,r_k} absent={k,v,w} closed
    return both["r_k"]                             #@ ok r_k


raw = pd.read_csv(io.StringIO(CSV))
pref = raw.add_prefix("in_")
# every column now starts with "in_": the old names are gone whatever the input held
# (a row lists absent keys it knows; x and y were never seen: checked by the access below)
#@todo row pref: absent={x,y} open
#@row pref: open
pref["in_x"]                                       #@ input in_x
try:
    pref["x"]                                      #@ missing x
except KeyError:
    pass

if __name__ == "__main__":
    for f in (pd.DataFrame({"a": [1]}), pd.DataFrame({"a": [1], "b": [2]})):
        try:
            pre(f)
        except KeyError:
            pass
    suf(pd.DataFrame({"a": [1], "b": [2]}))
    suf(pd.DataFrame({"a": [], "b": []}))
    pre_index(pd.DataFrame({"a": [1], "b": [2]}))
    pre_index(pd.DataFrame({"a": [1, 2], "b": [3, 4]}))
    side_by_side(pd.DataFrame({"k": [1], "v": [2]}), pd.DataFrame({"k": [1], "w": [3]}))
    side_by_side(pd.DataFrame({"k": [1]}), pd.DataFrame({"k": [2]}))

# pd.get_dummies: columns= encodes just those (removed, <col>_<value> added), prefix= / prefix_sep=, drop_first, a Series, the default (string columns)
import io

import pandas as pd

CSV = "id,color,size\n1,red,S\n2,blue,M\n"


def encode(df):
    d = pd.get_dummies(df, columns=["color"])
    #@row d: present={id,size,color_red,color_blue} definite={id,color_red} absent={color} closed
    d["color_red"]                                 #@ ok color_red
    d["color"]                                     #@ missing color


def prefixed(df):
    d = pd.get_dummies(df, columns=["color"], prefix="c", prefix_sep=":")
    #@row d: present={id,size,"c:red","c:blue"} definite={id,"c:red"} absent={color,color_red} closed
    return d


def first_dropped(df):
    d = pd.get_dummies(df, columns=["color"], drop_first=True)
    #@row d: present={id,size,color_red} definite={id} absent={color,color_blue} closed
    return d


def series(df):
    d = pd.get_dummies(df["color"])                #@ ok color
    #@row d: present={red,blue} definite={red} absent={color,id} closed
    d2 = pd.get_dummies(df["color"], prefix="is")
    #@row d2: present={is_red,is_blue} definite={is_red} absent={red} closed
    return d2


def default_cols(df):
    d = pd.get_dummies(df)
    #@row d: present={id,color_red,color_blue,size_S,size_M} definite={id,color_red} absent={color,size} closed
    return d


raw = pd.read_csv(io.StringIO(CSV))
enc = pd.get_dummies(raw, columns=["color"])
#@row enc: absent={color} open
enc["id"]                                          #@ input id
#@schema read_csv@L42: always={id}

if __name__ == "__main__":
    a = pd.DataFrame({"id": [1, 2], "color": ["red", "blue"]})
    b = pd.DataFrame({"id": [3], "color": ["red"], "size": ["S"]})
    c = pd.DataFrame({"id": [4, 5], "color": ["red", "red"], "size": ["S", "M"]})
    for f in (a, b, c):
        try:
            encode(f)
        except KeyError:
            pass
        prefixed(f)
        first_dropped(f)
        series(f)
        default_cols(f)

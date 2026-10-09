# renaming through the columns Index: df.columns = df.columns.str.lower() / .str.strip().str.replace(), Index.map, a literal list; the old names are gone
import io

import pandas as pd

CSV = "ID,Amount\n1,2\n"


def lowered(df):
    df.columns = df.columns.str.lower()
    #@row df: present={id,amount,note} definite={id,amount} absent={ID,Amount,Note} closed
    df["amount"]                                   #@ ok amount
    df["Amount"]                                   #@ missing Amount


def stripped(df):
    out = df.copy()
    out.columns = out.columns.str.strip().str.replace(" ", "_")
    #@row out: present={Unit_Price,qty} definite={Unit_Price} absent={" Unit Price ","Unit Price"} closed
    #@row df: present={" Unit Price ",qty} definite={" Unit Price "} absent={Unit_Price} closed
    return out["Unit_Price"]                       #@ ok Unit_Price


def mapped(df):
    df.columns = df.columns.map(str.upper)
    #@row df: present={A,B} definite={A} absent={a,b} closed
    return df["A"]                                 #@ ok A


def relisted(df):
    df.columns = ["x", "y"]
    #@row df: present={x,y} definite={x,y} absent={a,b} closed
    return df


raw = pd.read_csv(io.StringIO(CSV))
raw.columns = raw.columns.str.lower()
# lower-cased names cannot hold an upper-case letter
#@row raw: absent={ID,Amount} open
raw["amount"]                                      #@ input amount

if __name__ == "__main__":
    for f in (pd.DataFrame({"ID": [1], "Amount": [2]}), pd.DataFrame({"ID": [1], "Amount": [2], "Note": ["n"]})):
        try:
            lowered(f)
        except KeyError:
            pass
    stripped(pd.DataFrame({" Unit Price ": [1.0], "qty": [2]}))
    stripped(pd.DataFrame({" Unit Price ": [1.0]}))
    mapped(pd.DataFrame({"a": [1]}))
    mapped(pd.DataFrame({"a": [1], "b": [2]}))
    relisted(pd.DataFrame({"a": [1], "b": [2]}))
    relisted(pd.DataFrame({"a": [], "b": []}))

# melt: id_vars stay, value_vars fold into var_name / value_name (default "variable" / "value", or the columns' name), other columns go; pd.melt and DataFrame.melt
import io

import pandas as pd

CSV = "id,jan,feb\n1,2,3\n4,5,6\n"


def default_names(df):
    long = df.melt(id_vars=["id"])
    #@row long: present={id,variable,value} definite={id,variable,value} absent={x,y} closed
    long["value"]                                  #@ ok value
    long["x"]                                      #@ missing x


def named(df):
    long = pd.melt(df, id_vars="id", value_vars=["x"], var_name="metric", value_name="amount")
    #@row long: present={id,metric,amount} definite={id,metric,amount} absent={x,y,variable,value} closed
    return long


def no_ids(df):
    long = df.melt()
    #@row long: present={variable,value} definite={variable,value} absent={id,x} closed
    return long


def col_name(df):
    df.columns.name = "field"
    long = df.melt(id_vars=["id"])
    #@row long: present={id,field,value} definite={id,field,value} absent={variable} closed
    return long


raw = pd.read_csv(io.StringIO(CSV))
lng = raw.melt(id_vars=["id"], var_name="month", value_name="sales")
# the folded columns come from the data, but the result's columns do not
#@row lng: present={id,month,sales} definite={id,month,sales} absent={jan,feb} closed
lng["sales"]                                       #@ ok sales

if __name__ == "__main__":
    for f in (pd.DataFrame({"id": [1], "x": [2], "y": [3]}), pd.DataFrame({"id": [1], "x": [2]})):
        try:
            default_names(f)
        except KeyError:
            pass
        named(f.copy())
        no_ids(f.copy())
        col_name(f.copy())

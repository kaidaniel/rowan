# rename_axis names an axis, not a column: columns unchanged; reset_index then uses that name
import pandas as pd


def named_index(df):
    n = df.rename_axis("row_id")
    #@row n: present={a,b} definite={a} absent={row_id} closed
    back = n.reset_index()
    #@row back: present={row_id,a,b} definite={row_id,a} absent={index} closed
    back["row_id"]                               #@ ok row_id
    n["row_id"]                                  #@ missing row_id


def named_columns(df):
    c = df.rename_axis(columns="field")
    #@row c: present={a,b} definite={a} absent={field} closed
    c["a"]                                       #@ ok a
    plain = df.reset_index()
    #@row plain: present={index,a,b} definite={index,a} closed
    plain["index"]                               #@ ok index


frame = pd.DataFrame({"a": [1, 2]}, index=[10, 20])
k = frame.rename_axis("key").reset_index()       #@row k: present={key,a} definite={key,a} closed
k["key"]                                         #@ ok key
cleared = frame.rename_axis("key").rename_axis(None).reset_index()  #@row cleared: present={index,a} definite={index,a} absent={key} closed

if __name__ == "__main__":
    caught = []
    for df in [frame, pd.DataFrame({"a": [3], "b": [4]})]:
        try:
            named_index(df)
        except KeyError as e:
            caught.append(e)
        named_columns(df)

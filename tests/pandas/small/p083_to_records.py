# to_records: a numpy record array whose fields are the index name ("index") plus the columns
import pandas as pd


def fields(df):
    rec = df.to_records()
    names = dict.fromkeys(rec.dtype.names)               #@row names: present={index,a,b,c} definite={index,a} closed
    flat = df.to_records(index=False)
    names2 = dict.fromkeys(flat.dtype.names)             #@row names2: present={a,b,c} definite={a} absent={index} closed
    first = flat.a                                       #@ ok a
    return flat.dtype.fields["index"]                    #@ missing index


def keyed(df):
    rec = df.set_index("a").to_records()
    names = dict.fromkeys(rec.dtype.names)               #@row names: present={a,b,c} definite={a} absent={index} closed
    return rec.dtype.fields["c"]                         #@ ok c ; missing c


def back(df):
    again = pd.DataFrame.from_records(df.to_records(index=False))   #@row again: present={a,b,c} definite={a} closed
    return again


if __name__ == "__main__":
    caught = []  # keeps each raised exception alive
    f1 = pd.DataFrame({"a": [1, 2], "b": [3.0, 4.0]})
    f2 = pd.DataFrame({"a": [5], "c": [6]})
    for frame in (f1, f2):
        try:
            fields(frame)
        except KeyError as e:
            caught.append(e)
        back(frame)
    keyed(f2)
    try:
        keyed(f1)
    except KeyError as e:
        caught.append(e)

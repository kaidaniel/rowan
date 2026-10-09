# inplace=True on drop / rename / sort_values / fillna / set_index: each returns None and changes the receiver (also the caller's frame)
import pandas as pd


def edit(df):
    r1 = df.drop(columns=["tmp"], inplace=True)  #@value r1: exactly={None}
    #@row df: present={id,v,name} definite={id,v,name} absent={tmp} closed
    r2 = df.rename(columns={"v": "value"}, inplace=True)   #@value r2: exactly={None}
    #@row df: present={id,value,name} definite={id,value,name} absent={tmp,v} closed
    r3 = df.sort_values("value", inplace=True)   #@ ok value
    #@value r3: exactly={None}
    r4 = df.fillna({"name": ""}, inplace=True)   #@value r4: exactly={None}
    #@row df: present={id,value,name} definite={id,value,name} closed
    r5 = df.set_index("id", inplace=True)        #@value r5: exactly={None}
    #@row df: present={value,name} definite={value,name} absent={id,tmp,v} closed
    return df


def old_name(df):
    return df["v"]                               #@ missing v


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for f in (pd.DataFrame({"id": [1, 2], "v": [3, 1], "name": ["a", None], "tmp": [0, 0]}),
              pd.DataFrame({"id": [], "v": [], "name": [], "tmp": []})):
        edit(f)
        f["value"]                               #@ ok value
        #@row f: present={value,name} definite={value,name} absent={id,tmp,v} closed
        try:
            old_name(f)
        except KeyError as e:
            kept.append(e)

# value_counts / size: to_dict keys are data values; reset_index gives the key column plus count / proportion / 0
import pandas as pd


def tallies(df):
    counts = df["k"].value_counts().to_dict()            #@ ok k
    #@row counts: present={x,y,z} definite={x} closed
    vc = df[["k"]].value_counts().reset_index()          #@row vc: present={k,count} definite={k,count} closed
    share = df["k"].value_counts(normalize=True).reset_index()   #@row share: present={k,proportion} definite={k,proportion} absent={count} closed
    sizes = df.groupby("k").size().reset_index()         #@row sizes: present={k,0} definite={k,0} closed
    named = df.groupby("k").size().reset_index(name="n")   #@row named: present={k,n} definite={k,n} absent={0} closed
    n = counts.get("y", 0)                               #@value n: exactly={0,1}
    return counts["y"]                                   #@ possibly y


if __name__ == "__main__":
    caught = []  # keeps each raised exception alive
    f1 = pd.DataFrame({"k": ["x", "y", "x"]})
    f2 = pd.DataFrame({"k": ["x", "z"], "v": [1, 2]})
    for frame in (f1, f2):
        try:
            tallies(frame)
        except KeyError as e:
            caught.append(e)

# describe(): columns = the numeric columns, index = the statistics; include="all" adds the other columns and unique/top/freq
import pandas as pd


def stats(df):
    d = df.describe()                                    #@row d: present={a,b} definite={a} absent={name} closed
    d["a"]                                               #@ ok a
    means = d.loc["mean"]                                #@row means: present={a,b} definite={a} closed
    t = d.T                                              #@row t: present={count,mean,std,min,"25%","50%","75%",max} definite={count,mean,std,min,"25%","50%","75%",max} closed
    full = df.describe(include="all")                    #@row full: present={a,b,name} definite={a,name} closed
    ft = full.T                                          #@row ft: present={count,unique,top,freq,mean,std,min,"25%","50%","75%",max} definite={count,unique,top,freq,mean,std,min,"25%","50%","75%",max} closed
    words = df[["name"]].describe().T                    #@row words: present={count,unique,top,freq} definite={count,unique,top,freq} absent={mean} closed
    return d["name"]                                     #@ missing name


if __name__ == "__main__":
    caught = []  # keeps each raised exception alive
    f1 = pd.DataFrame({"a": [1, 2, 3], "b": [0.5, 1.5, 2.5], "name": ["x", "y", "x"]})
    f2 = pd.DataFrame({"a": [4, 5], "name": ["p", "q"]})
    for frame in (f1, f2):
        try:
            stats(frame)
        except KeyError as e:
            caught.append(e)

# pd.DataFrame from a dict of columns: keys are the columns; columns= selects and NaN-fills
import pandas as pd


def from_cols(n, extra):
    df = pd.DataFrame({"id": list(range(n)), "name": ["x"] * n})
    #@row df: present={id,name} definite={id,name} closed
    if extra:
        df["note"] = ""
    df["name"]                                   #@ ok name
    #@row df: present={id,name,note} definite={id,name} closed
    df["note"]                                   #@ ok note ; missing note


def selected(data):
    df = pd.DataFrame(data, columns=["a", "b"])
    #@row df: present={a,b} definite={a,b} absent={c,d} closed
    n = len(df.columns)                          #@value n: exactly={2}
    df["b"]                                      #@ ok b
    df["c"]                                      #@ missing c


base = pd.DataFrame({"k": [1, 2], "v": [3.0, 4.0]})  #@row base: present={k,v} definite={k,v} closed
base["k"]                                        #@ ok k
empty = pd.DataFrame({})                         #@row empty: present={} closed
try:
    empty["k"]                                   #@ missing k
except KeyError:
    pass

if __name__ == "__main__":
    caught = []
    for n, extra in [(2, True), (0, False), (3, True)]:
        try:
            from_cols(n, extra)
        except KeyError as e:
            caught.append(e)
    for data in [{"a": [1], "c": [2]}, {"a": [1], "b": [2], "d": [3]}, {"b": [5, 6]}]:
        try:
            selected(data)
        except KeyError as e:
            caught.append(e)

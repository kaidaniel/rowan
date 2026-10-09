# str.split(expand=True) / partition: integer columns 0, 1, …; n=1 bounds them to {0, 1}, partition always gives 3
import pandas as pd


def split_name(df):
    parts = df["name"].str.split(" ", n=1, expand=True)  #@ ok name
    #@row parts: present={0,1} definite={0} closed
    first = parts[0]                                     #@ ok 0
    return parts[1]                                      #@ possibly 1


def partitions(df):
    tri = df["name"].str.partition(" ")                  #@row tri: present={0,1,2} definite={0,1,2} closed
    back = df["name"].str.rpartition(" ")                #@row back: present={0,1,2} definite={0,1,2} closed
    free = df["name"].str.split(" ", expand=True)        #@row free: definite={0} open
    return tri[3]                                        #@ missing 3


if __name__ == "__main__":
    caught = []  # keeps each raised exception alive
    f1 = pd.DataFrame({"name": ["ada lovelace", "alan turing"]})
    f2 = pd.DataFrame({"name": ["plato", "homer"], "age": [1, 2]})
    f3 = pd.DataFrame({"name": ["grace brewster hopper", "kurt"]})
    for frame in (f1, f2, f3):
        try:
            split_name(frame)
        except KeyError as e:
            caught.append(e)
        try:
            partitions(frame)
        except KeyError as e:
            caught.append(e)

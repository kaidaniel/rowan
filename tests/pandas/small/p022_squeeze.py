# squeeze: a one-row frame squeezed on axis 0 is a Series indexed by the columns (a record); a 1 x 1 frame its cell, a numpy scalar (was `exactly={5}`: an int, whose methods are not its)
import pandas as pd


def one_row(df):
    rec = df.head(1).squeeze(axis=0)
    #@row rec: present={id,name,score} definite={id,name} closed
    rec["name"]                                  #@ ok name
    rec["zz"]                                    #@ missing zz


def wide_stays(df):
    same = df.squeeze(axis=1)                    # more than one column: the frame itself
    #@row same: present={id,name,score} definite={id,name} closed
    same["id"]                                   #@ ok id


frame = pd.DataFrame({"id": [1], "name": ["a"]})
r = frame.squeeze(axis=0)                        #@row r: present={id,name} definite={id,name} closed
r["id"]                                          #@ ok id
first = frame.iloc[0]                            #@row first: present={id,name} definite={id,name} closed
cell = pd.DataFrame({"x": [5]}).squeeze()        #@value cell: includes={5}
col = frame[["name"]].squeeze(axis=1)            # one column: a Series over the row index

if __name__ == "__main__":
    caught = []
    for df in [frame, pd.DataFrame({"id": [2, 3], "name": ["b", "c"], "score": [1.0, 2.0]})]:
        try:
            one_row(df)
        except KeyError as e:
            caught.append(e)
        wide_stays(df)

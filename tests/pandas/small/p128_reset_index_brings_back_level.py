# reset_index moves the index levels back into columns: a level named like a column dropped
# before brings that column back (the frames' index is named "t" and they also hold a column
# "t"). With the levels' names not known (concatenated frames from unknown code), a column
# dropped before may be there again: rowan said "t" was missing for sure (sktime's gluonts
# adapter). Expectations written from the program text and pandas' documented behaviour.
import pandas as pd


def merge(frames):
    dfs = pd.concat(frames, keys=range(len(frames)))
    dfs = dfs.drop("t", axis=1)
    m = dfs.reset_index()
    return m["t"]  #@ sound t


a = pd.DataFrame({"t": [1, 2], "v": [3, 4]}, index=pd.Index([10, 11], name="t"))
col = merge([a, a, a])

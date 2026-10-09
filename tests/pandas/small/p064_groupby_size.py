# groupby().size(): a Series (reset_index gives the key and 0, or name=); as_index=False: a frame {key, size}
import pandas as pd

A = pd.DataFrame({"team": ["r", "b", "r"], "pts": [1, 2, 3]})
B = pd.DataFrame({"team": ["g"], "pts": [4], "bonus": [1]})


def sizes(df):
    s = df.groupby("team").size()
    r = s.reset_index()
    #@row r: present={team,0} definite={team,0} closed
    n = s.reset_index(name="n")
    #@row n: present={team,n} definite={team,n} absent={0,size} closed
    f = df.groupby("team", as_index=False).size()
    #@row f: present={team,size} definite={team,size} closed
    f["size"]                                            #@ ok size
    return f


def pts(f):
    return f["pts"]                                      #@ missing pts


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for df in (A, B):
        try:
            pts(sizes(df))
        except KeyError as e:
            kept.append(e)

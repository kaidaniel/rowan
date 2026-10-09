# isin / between: masks keep the frame's columns; df.isin(...) is a bool frame with the same columns
import pandas as pd


def keep(df, codes):
    hit = df[df["code"].isin(codes)]             #@ ok code
    #@row hit: present={code,amt,day} definite={code,amt} closed
    mid = hit[hit["amt"].between(1, 10)]         #@ ok amt
    #@row mid: present={code,amt,day} definite={code,amt} closed
    flags = df.isin({"code": codes})             # dict keys not columns are ignored, all columns stay
    #@row flags: present={code,amt,day} definite={code,amt} closed
    mid["day"]                                   #@ ok day ; missing day


def outside(df):
    out = df[~df["amt"].between(0, 5, inclusive="left")]  #@ ok amt
    #@row out: present={code,amt,day} definite={code,amt} closed
    both = df.isin([1, "a"])
    #@row both: present={code,amt,day} definite={code,amt} closed
    both["zz"]                                   #@ missing zz


frame = pd.DataFrame({"code": ["a", "b"], "amt": [3, 30]})
sel = frame[frame.code.isin(["a"])]              #@row sel: present={code,amt} definite={code,amt} closed
sel["amt"]                                       #@ ok amt
inside = frame.amt.between(0, 10).sum()

if __name__ == "__main__":
    caught = []
    for df, codes in [(frame, ["a"]), (pd.DataFrame({"code": ["x"], "amt": [2], "day": [1]}), []), (pd.DataFrame({"code": [], "amt": [], "day": []}), ["z"])]:
        try:
            keep(df, codes)
        except KeyError as e:
            caught.append(e)
        try:
            outside(df)
        except KeyError as e:
            caught.append(e)

# Series.map / replace into new columns; DataFrame.replace keeps the columns; a mapped Series to_frame has one column
import pandas as pd


def grade(score):
    return "hi" if score > 5 else "lo"


def recoded(df):
    out = df.copy()
    out["grade"] = out["score"].map(grade)               #@ ok score
    out["flag"] = out["code"].replace({"A": 1, "B": 2})  #@ ok code
    out = out.replace({"code": {"A": "alpha"}})          #@row out: present={score,code,note,grade,flag} definite={score,code,grade,flag} closed
    hi = out["grade"].map({"hi": 1}).to_frame("hi")      #@row hi: present={hi} definite={hi} absent={grade} closed
    return out["note"]                                   #@ ok note ; missing note


def renamed_codes(df):
    s = df["code"].replace("A", "alpha").rename("label")
    f = s.to_frame()                                     #@row f: present={label} definite={label} absent={code} closed
    return f["code"]                                     #@ missing code


if __name__ == "__main__":
    caught = []  # keeps each raised exception alive
    f1 = pd.DataFrame({"score": [3, 8], "code": ["A", "B"], "note": ["x", "y"]})
    f2 = pd.DataFrame({"score": [9], "code": ["C"]})
    for frame in (f1, f2):
        try:
            recoded(frame)
        except KeyError as e:
            caught.append(e)
        try:
            renamed_codes(frame)
        except KeyError as e:
            caught.append(e)

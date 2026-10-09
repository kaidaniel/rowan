# pd.wide_to_long: stub columns (A1970, A1980) fold into the stub name with j as a new index level; i joins the index; other columns stay
import pandas as pd


def fold(df):
    long = pd.wide_to_long(df, stubnames="A", i="id", j="year")
    #@row long: present={A,X} definite={A} absent={id,year,A1970,A1980} closed
    long["A"]                                      #@ ok A
    flat = long.reset_index()
    #@row flat: present={id,year,A,X} definite={id,year,A} closed
    long["A1970"]                                  #@ missing A1970


def two_stubs(df):
    long = pd.wide_to_long(df, stubnames=["A", "B"], i="id", j="yr", sep="_")
    #@row long: present={A,B} definite={A,B} absent={A_1,A_2,B_1,B_2,id,yr} closed
    return long.reset_index()


def word_suffix(df):
    long = pd.wide_to_long(df, ["score"], i="id", j="term", sep="_", suffix=r"\w+")
    #@row long: present={score,name} definite={score} absent={score_fall,score_spring,term,id} closed
    return long


if __name__ == "__main__":
    for f in (pd.DataFrame({"id": [1, 2], "A1970": [1.0, 2.0], "A1980": [3.0, 4.0], "X": [5, 6]}),
              pd.DataFrame({"id": [3], "A1970": [7.0], "A1980": [8.0]})):
        try:
            fold(f)
        except KeyError:
            pass
    two_stubs(pd.DataFrame({"id": [1], "A_1": [1], "A_2": [2], "B_1": [3], "B_2": [4]}))
    two_stubs(pd.DataFrame({"id": [1, 2], "A_1": [1, 5], "A_2": [2, 6], "B_1": [3, 7], "B_2": [4, 8]}))
    word_suffix(pd.DataFrame({"id": [1], "name": ["a"], "score_fall": [1], "score_spring": [2]}))
    word_suffix(pd.DataFrame({"id": [2], "score_fall": [3], "score_spring": [4]}))

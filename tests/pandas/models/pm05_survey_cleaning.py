# Survey cleaning: wide survey with optional questions; header validation, rename via a mapping, recoding with map / replace / where / cut / qcut, get_dummies, melt to long, groupby agg list, pivot / pivot_table, crosstab with margins, columns assignment
import io

import pandas as pd

REQUIRED = ["respondent_id", "Q1_age", "Q2_gender", "Q3_satisfaction", "Q4_recommend"]
RENAME = {"Q1_age": "age", "Q2_gender": "gender", "Q3_satisfaction": "satisfaction", "Q4_recommend": "recommend", "Q5_comments": "comments", "Q6_income": "income"}
SATISFACTION = {"very dissatisfied": 1, "dissatisfied": 2, "neutral": 3, "satisfied": 4, "very satisfied": 5}
GENDER = {"m": "male", "f": "female"}

SURVEY_A = """respondent_id,Q1_age,Q2_gender,Q3_satisfaction,Q4_recommend,Q5_comments
1,34,M,very satisfied,Yes,great
2,51,f,neutral,No,
3,,F,4,yes,ok
4,29,x,dissatisfied,No,slow
4,29,x,dissatisfied,No,slow
"""

# an income question instead of comments, and an extra source column
SURVEY_B = """respondent_id,Q1_age,Q2_gender,Q3_satisfaction,Q4_recommend,Q6_income,source
10,22,m,2,no,25000,web
11,67,F,very satisfied,YES,,mail
12,45,f,satisfied, Yes ,90000,web
"""

# required questions only; an impossible age
SURVEY_C = """respondent_id,Q1_age,Q2_gender,Q3_satisfaction,Q4_recommend
20,130,M,1,No
21,40,,5,Yes
"""


def load_survey(text):
    raw = pd.read_csv(io.StringIO(text))
    for col in REQUIRED:
        if col not in raw.columns:
            raise KeyError(col)
    raw = raw.drop_duplicates("respondent_id")
    return raw  #@row raw: definite={respondent_id,Q1_age,Q2_gender,Q3_satisfaction,Q4_recommend} open


def rename_questions(raw):
    df = raw.rename(columns=RENAME)
    #@row df: definite={respondent_id,age,gender,satisfaction,recommend} absent={Q1_age,Q2_gender,Q3_satisfaction,Q4_recommend,Q5_comments,Q6_income} open
    has_income = "income" in df.columns
    #@value has_income: exactly={True, False}
    return df


def stale_header(df):
    return df["Q1_age"]                                                       #@ missing Q1_age


def recode(df):
    df = df.copy()
    df["age"] = pd.to_numeric(df["age"], errors="coerce")                     #@ ok age
    df["age"] = df["age"].where(df["age"].between(16, 110))
    df["gender"] = df["gender"].str.lower().map(GENDER).fillna("other")       #@ ok gender
    sat = df["satisfaction"].astype(str).str.strip().str.lower()              #@ ok satisfaction
    df["satisfaction"] = sat.map(SATISFACTION).fillna(pd.to_numeric(sat, errors="coerce"))
    df["recommend"] = df["recommend"].str.strip().str.lower().replace({"yes": "Y", "no": "N"}).eq("Y")  #@ ok recommend
    df["age_band"] = pd.cut(df["age"], bins=[15, 30, 50, 110], labels=["16-30", "31-50", "51+"]).astype(str)
    if "income" in df.columns:
        df["income_band"] = pd.qcut(df["income"], q=2, labels=["low", "high"]).astype(str)  #@ ok income
    if "comments" in df.columns:
        df["has_comment"] = df["comments"].notna()                            #@ ok comments
    else:
        df["has_comment"] = False
    return df  #@row df: definite={respondent_id,age,gender,satisfaction,recommend,age_band,has_comment} absent={Q1_age,Q2_gender,Q3_satisfaction,Q4_recommend,Q5_comments,Q6_income} open


def income_median(df):
    return df["income"].median()                                              #@ possibly income


def comment_lengths(df):
    return df["comments"].str.len()                                           #@ possibly comments


def income_bands(df):
    return df["income_band"].value_counts()                                   #@ possibly income_band


def encode(df):
    enc = pd.get_dummies(df, columns=["gender", "age_band"], prefix=["g", "age"], dtype=int)
    #@row enc: definite={respondent_id,age,satisfaction,recommend,has_comment} absent={gender,age_band} open
    enc["age"]                                                                #@ ok age
    return enc


def gender_after_encoding(enc):
    return enc["gender"]                                                      #@ missing gender


def to_long(df):
    scored = df.assign(recommend_score=df["recommend"].astype(int) * 10)
    #@row scored: definite={respondent_id,age,gender,satisfaction,recommend,age_band,has_comment,recommend_score} open
    long = scored.melt(id_vars=["respondent_id", "age_band"], value_vars=["satisfaction", "recommend_score"], var_name="question", value_name="score")
    #@row long: present={respondent_id,age_band,question,score} definite={respondent_id,age_band,question,score} absent={satisfaction,recommend_score,variable,value} closed
    width = long.shape[1]
    #@value width: exactly={4}
    return long


def long_value(long):
    return long["value"]                                                      #@ missing value


def summarise(long):
    per_q = long.groupby("question")["score"].agg(["mean", "count"]).reset_index()
    #@row per_q: present={question,mean,count} definite={question,mean,count} absent={score} closed
    per_q["reliable"] = per_q["count"] >= 3                                   #@ ok count
    by_band = long.pivot_table(index="age_band", columns="question", values="score", aggfunc="mean")
    #@row by_band: open
    by_band = by_band.reindex(columns=["satisfaction", "recommend_score"])
    #@row by_band: present={satisfaction,recommend_score} definite={satisfaction,recommend_score} closed
    by_band["gap"] = by_band["recommend_score"] / 2 - by_band["satisfaction"]  #@ ok recommend_score ; ok satisfaction
    wide = long.pivot(index="respondent_id", columns="question", values="score").reset_index()
    #@row wide: definite={respondent_id} open
    wide["satisfaction"]                                                      #@ sound satisfaction
    return per_q  #@row per_q: present={question,mean,count,reliable} definite={question,mean,count,reliable} closed


def crosstab_summary(df):
    ct = pd.crosstab(df["age_band"], df["recommend"], normalize="index")      #@ ok age_band ; ok recommend
    #@row ct: open
    ct = ct.rename(columns={True: "promoter", False: "detractor"})
    ct = ct.reindex(columns=["promoter", "detractor"], fill_value=0.0)
    #@row ct: present={promoter,detractor} definite={promoter,detractor} absent={True,False} closed
    ct["nps"] = (ct["promoter"] - ct["detractor"]) * 100                      #@ ok promoter ; ok detractor
    by_gender = pd.crosstab(df["gender"], df["age_band"], margins=True)
    #@row by_gender: definite={All} open
    total = by_gender["All"].iloc[-1]                                         #@ ok All
    return ct


def export(df):
    out = df[["respondent_id", "age", "satisfaction", "recommend"]].copy()
    out.columns = ["id", "age_years", "sat", "promoter"]
    #@row out: present={id,age_years,sat,promoter} definite={id,age_years,sat,promoter} absent={respondent_id,age} closed
    records = out.to_dict("records")
    for rec in records:
        rec["id"]                                                             #@ ok id
        #@row rec: present={id,age_years,sat,promoter} definite={id,age_years,sat,promoter} closed
    return records


def exported_age(df):
    out = df[["respondent_id", "age"]].copy()
    out.columns = ["id", "age_years"]
    return out["age"]                                                         #@ missing age


def run(text):
    df = rename_questions(load_survey(text))
    try:
        stale_header(df)
    except KeyError:
        pass
    df = recode(df)
    for probe in (income_median, comment_lengths, income_bands):
        try:
            probe(df)
        except KeyError:
            pass
    enc = encode(df)
    try:
        gender_after_encoding(enc)
    except KeyError:
        pass
    long = to_long(df)
    try:
        long_value(long)
    except KeyError:
        pass
    summarise(long)
    crosstab_summary(df)
    export(df)
    try:
        exported_age(df)
    except KeyError:
        pass
    return df


if __name__ == "__main__":
    for text in (SURVEY_A, SURVEY_B, SURVEY_C):
        run(text)

#@schema read_csv@L34: always={respondent_id,Q1_age,Q2_gender,Q3_satisfaction,Q4_recommend} sometimes={Q5_comments,Q6_income} only={respondent_id,Q1_age,Q2_gender,Q3_satisfaction,Q4_recommend}

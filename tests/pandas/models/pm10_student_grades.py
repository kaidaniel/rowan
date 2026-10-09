"""Student grades: long-format grades -> pivot_table per student x subject, missing subjects
filled in a loop, weighted averages via assign with named functions, rank / pd.cut, merge with
the student list, melt back to long, to_dict("records") report rows with dict access; run on
two grade books (different subjects, extra columns) and two student lists (advisor or not)."""
import io

import pandas as pd

REQUIRED = ["math", "physics", "english"]
WEIGHTS = {"math": 0.5, "physics": 0.3, "english": 0.2}

GRADES_1 = """student,subject,term,score
s1,Math,T1,91
s1,Physics,T1,78
s1,English,T1,85
s1,Art,T1,99
s2,Math,T1,55
s2,Physics,T1,61
s2,English,T1,
s2,Math,T2,65
s3,Math,T1,72
s3,English,T1,88
s3,Physics,T2,70
"""

GRADES_2 = """student,subject,term,score,comment
s7,math,T1,80,
s7,english,T1,90,late
s8,math,T1,40,
s8,english,T1,95,
s9,math,T2,100,
"""

STUDENTS_1 = """student,name,cohort,advisor
s1,ada,2024,dr. noether
s2,bob,2024,dr. curie
s3,cy,2025,dr. noether
"""

STUDENTS_2 = """student,name,cohort,email
s7,dee,2023,dee@example.org
s8,eli,2023,eli@example.org
s9,fay,2024,fay@example.org
"""


def load_grades(text, terms):
    grades = pd.read_csv(io.StringIO(text))
    grades["student"] = grades["student"].str.strip()                   #@ input student
    grades["subject"] = grades["subject"].str.lower()                   #@ input subject
    grades["score"] = pd.to_numeric(grades["score"], errors="coerce")   #@ input score
    grades = grades[grades["score"].notna() & grades["term"].isin(terms)]  #@ ok score ; input term
    #@row grades: present={student,subject,score,term} definite={student,subject,score,term} open
    return grades


def to_wide(grades):
    wide = grades.pivot_table(index="student", columns="subject", values="score", aggfunc="mean")
    #@row wide: present={} definite={} open
    for subj in REQUIRED:
        if subj not in wide.columns:
            wide[subj] = float("nan")
    n_subjects = len(wide.columns)  #@row wide: present={math,physics,english} definite={math,physics,english} open
    wide = wide[REQUIRED]
    #@row wide: present={math,physics,english} definite={math,physics,english} closed
    wide.columns.name = None
    return wide


def weighted(d):
    return sum(d[s].fillna(0.0) * w for s, w in WEIGHTS.items())


def count_missing(d):
    return d[REQUIRED].isna().sum(axis=1)


def score_table(wide):
    scores = wide.assign(weighted=weighted, n_missing=count_missing)
    #@row scores: present={math,physics,english,weighted,n_missing} definite={math,physics,english,weighted,n_missing} closed
    scores["rank"] = scores["weighted"].rank(ascending=False, method="min").astype(int)  #@ ok weighted
    scores["pct_rank"] = scores["weighted"].rank(pct=True)
    scores["letter"] = pd.cut(scores["weighted"], bins=[0, 60, 70, 80, 90, 101], labels=["F", "D", "C", "B", "A"], right=False)
    subject_ranks = scores[REQUIRED].rank(axis=0, ascending=False)     #@ ok math ; ok physics ; ok english
    #@row subject_ranks: present={math,physics,english} definite={math,physics,english} closed
    scores = scores.join(subject_ranks.add_suffix("_rank"))
    #@row scores: present={math,physics,english,weighted,n_missing,rank,pct_rank,letter,math_rank,physics_rank,english_rank} definite={math,physics,english,weighted,n_missing,rank,pct_rank,letter,math_rank,physics_rank,english_rank} closed
    return scores


def probe_art(scores):
    # only the required subjects survive the projection
    return scores["art"]                                                #@ missing art


def load_students(text):
    students = pd.read_csv(io.StringIO(text))
    students["student"] = students["student"].str.strip()               #@ input student
    students["name"] = students["name"].str.title()                     #@ input name
    students["cohort"] = students["cohort"].astype(str)                 #@ input cohort
    cols = ["student", "name", "cohort"]
    if "advisor" in students.columns:
        students["advisor_name"] = students["advisor"].str.title()      #@ ok advisor
        cols.append("advisor_name")
    students = students[cols]
    #@row students: present={student,name,cohort,advisor_name} definite={student,name,cohort} absent={advisor} closed
    return students


def build_report(scores, students):
    report = scores.reset_index().merge(students, on="student", how="left")
    #@row report: present={student,name,cohort,advisor_name,math,physics,english,weighted,n_missing,rank,pct_rank,letter,math_rank,physics_rank,english_rank} definite={student,name,cohort,math,physics,english,weighted,n_missing,rank,pct_rank,letter,math_rank,physics_rank,english_rank} closed
    report = report.sort_values("rank")                                 #@ ok rank
    by_cohort = report.groupby("cohort").agg(mean_weighted=("weighted", "mean"), best_rank=("rank", "min"), size=("student", "count"))  #@ ok cohort
    #@row by_cohort: present={mean_weighted,best_rank,size} definite={mean_weighted,best_rank,size} absent={cohort,weighted} closed
    long_again = report[["student", "math", "physics", "english"]].melt(id_vars="student", var_name="subject", value_name="score")
    #@row long_again: present={student,subject,score} definite={student,subject,score} absent={math} closed
    return report, by_cohort


def probe_advisor(report):
    # only student lists with an advisor column carry the name
    return report["advisor_name"]                                       #@ possibly advisor_name


def report_rows(report):
    rows = []
    for rec in report.to_dict("records"):
        grade = str(rec["letter"])                                      #@ ok letter
        #@value grade: exactly={"A","B","C","D","F"}
        status = "complete" if rec["n_missing"] == 0 else "incomplete"  #@ ok n_missing
        #@value status: exactly={"complete","incomplete"}
        entry = {"student": rec["student"], "name": rec["name"], "grade": grade, "status": status}  #@ ok student ; ok name
        if "advisor_name" in rec:
            entry["advisor"] = rec["advisor_name"]                      #@ ok advisor_name
        rows.append(entry)  #@row entry: present={student,name,grade,status,advisor} definite={student,name,grade,status} closed
    return rows


def probe_entry(rows):
    # report rows carry the letter as "grade"
    return rows[0]["letter"]                                            #@ missing letter


def run(grades_text, students_text, terms):
    grades = load_grades(grades_text, terms)
    wide = to_wide(grades)
    scores = score_table(wide)
    students = load_students(students_text)
    report, by_cohort = build_report(scores, students)
    rows = report_rows(report)
    for probe, arg in ((probe_art, scores), (probe_advisor, report), (probe_entry, rows)):
        try:
            probe(arg)
        except KeyError:
            pass
    return rows, by_cohort


if __name__ == "__main__":
    for g, s, terms in [(GRADES_1, STUDENTS_1, ["T1", "T2"]), (GRADES_2, STUDENTS_2, ["T1"]), (GRADES_1, STUDENTS_1, ["T1"])]:
        rows, by_cohort = run(g, s, terms)
        print(rows[0], by_cohort.shape)
#@schema read_csv@L48: always={student,subject,score,term} only={student,subject,score,term}
#@schema read_csv@L97: always={student,name,cohort} only={student,name,cohort}

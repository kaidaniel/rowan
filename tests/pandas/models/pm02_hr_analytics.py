# HR analytics: employees (open, optional columns) + departments (usecols) + salary history; latest salary per employee, merge_asof at review dates, tenure features, named aggregations, rank within department
import io

import pandas as pd

AS_OF = pd.Timestamp("2025-01-01")

EMPLOYEES_A = """emp_id,first,last,dept_id,hire_date,manager_id,remote
1,Ada,Lovelace,10,2015-03-01,,1
2,Alan,Turing,10,2019-06-15,1,0
3,Grace,Hopper,20,2021-01-10,1,1
4,Edsger,Dijkstra,20,2023-09-01,3,0
5,Barbara,Liskov,30,2018-11-20,1,0
"""

# no remote flag; the employee's own office location (overlaps the department's)
EMPLOYEES_B = """emp_id,first,last,dept_id,hire_date,manager_id,location
6,Ken,Thompson,10,2012-02-01,,NYC
7,Dennis,Ritchie,10,2016-08-08,6,NJ
8,Margaret,Hamilton,30,2020-04-04,6,BOS
"""

# no manager column
EMPLOYEES_C = """emp_id,first,last,dept_id,hire_date,remote
9,Linus,Torvalds,20,2022-02-02,1
10,Guido,Rossum,30,2014-05-05,0
"""

DEPARTMENTS_A = """dept_id,dept_name,location
10,Research,London
20,Compilers,Arlington
30,Systems,Boston
"""

DEPARTMENTS_B = """dept_id,budget,dept_name,location,head
10,1000000,Research,London,Ada
20,500000,Compilers,Arlington,Grace
30,750000,Systems,Boston,Barbara
"""

SALARIES_A = """emp_id,effective_date,salary,bonus
1,2015-03-01,100000,
1,2020-01-01,130000,5000
2,2019-06-15,90000,
3,2021-01-10,110000,2000
3,2023-01-01,120000,
4,2023-09-01,80000,
5,2018-11-20,105000,
"""

SALARIES_B = """emp_id,effective_date,salary
6,2012-02-01,120000
7,2016-08-08,95000
7,2022-01-01,n/a
8,2020-04-04,99000
"""

SALARIES_C = """emp_id,effective_date,salary,bonus,currency
9,2022-02-02,115000,0,EUR
10,2014-05-05,140000,10000,EUR
10,2024-01-01,150000,,EUR
"""


def load_employees(text):
    e = pd.read_csv(io.StringIO(text))
    e["emp_id"] = e["emp_id"].astype(int)                         #@ input emp_id
    e["dept_id"] = e["dept_id"].astype(int)                       #@ input dept_id
    e["name"] = e["first"].str.cat(e["last"], sep=" ")            #@ input first ; input last
    e = e.drop(columns=["first", "last"])
    e["hire_date"] = pd.to_datetime(e["hire_date"])               #@ input hire_date
    if "remote" not in e.columns:
        e["remote"] = 0
    e["remote"] = e["remote"].astype(bool)                        #@ ok remote
    return e  #@row e: definite={emp_id,dept_id,name,hire_date,remote} absent={first,last} open


def initials_of(e):
    return e["first"].str[0]                                      #@ missing first


def load_departments(text):
    d = pd.read_csv(io.StringIO(text), usecols=["dept_id", "dept_name", "location"])
    #@row d: present={dept_id,dept_name,location} definite={dept_id,dept_name,location} absent={budget,head} closed
    d["dept_id"] = d["dept_id"].astype(int)                       #@ ok dept_id
    d["dept_name"] = d["dept_name"].str.strip()                   #@ ok dept_name
    d["location"] = d["location"].str.upper()                     #@ ok location
    return d


def load_salaries(text):
    s = pd.read_csv(io.StringIO(text))
    s["emp_id"] = s["emp_id"].astype(int)                         #@ input emp_id
    s["effective_date"] = pd.to_datetime(s["effective_date"])     #@ input effective_date
    s["salary"] = pd.to_numeric(s["salary"], errors="coerce")     #@ input salary
    s = s.dropna(subset=["salary"])
    if "bonus" in s.columns:
        s["total_comp"] = s["salary"] + s["bonus"].fillna(0)      #@ ok bonus
    else:
        s["total_comp"] = s["salary"]
    s = s[["emp_id", "effective_date", "salary", "total_comp"]].sort_values(["emp_id", "effective_date"])
    #@row s: present={emp_id,effective_date,salary,total_comp} definite={emp_id,effective_date,salary,total_comp} absent={bonus,currency} closed
    return s


def latest_salary(s):
    idx = s.groupby("emp_id")["effective_date"].idxmax()
    latest = s.loc[idx].reset_index(drop=True)
    #@row latest: present={emp_id,effective_date,salary,total_comp} definite={emp_id,effective_date,salary,total_comp} absent={index} closed
    latest = latest.rename(columns={"salary": "current_salary", "effective_date": "salary_since"})
    #@row latest: present={emp_id,salary_since,current_salary,total_comp} definite={emp_id,salary_since,current_salary,total_comp} absent={salary,effective_date} closed
    return latest


def old_salary(latest):
    return latest["salary"]                                       #@ missing salary


def build_roster(e, d, latest):
    r = e.merge(d, on="dept_id", how="left", suffixes=("", "_dept"))
    #@row r: definite={emp_id,dept_id,name,hire_date,remote,dept_name,location} open
    r = r.filter(items=["emp_id", "name", "dept_id", "dept_name", "location", "location_dept", "hire_date", "remote", "manager_id"])
    #@row r: present={emp_id,name,dept_id,dept_name,location,location_dept,hire_date,remote,manager_id} definite={emp_id,name,dept_id,dept_name,location,hire_date,remote} closed
    r = r.merge(latest, on="emp_id", how="left")
    #@row r: present={emp_id,name,dept_id,dept_name,location,location_dept,hire_date,remote,manager_id,salary_since,current_salary,total_comp} definite={emp_id,name,dept_id,dept_name,location,hire_date,remote,salary_since,current_salary,total_comp} absent={salary,first} closed
    r["location"]                                                 #@ ok location
    return r


def office_of_dept(r):
    return r["location_dept"]                                     #@ possibly location_dept


def managers(r):
    return r["manager_id"].dropna().astype(int).unique()          #@ possibly manager_id


def team_sizes(r):
    if "manager_id" not in r.columns:
        return {}
    sizes = r.groupby("manager_id")["emp_id"].count()             #@ ok manager_id ; ok emp_id
    return sizes.to_dict()


def tenure_features(r, as_of):
    r = r.copy()
    r["tenure_years"] = (as_of - r["hire_date"]).dt.days / 365.25                    #@ ok hire_date
    r["tenure_band"] = pd.cut(r["tenure_years"], bins=[0, 2, 5, 100], labels=["junior", "mid", "senior"]).astype(str)
    r["raise_pending"] = r["salary_since"] < as_of - pd.Timedelta(days=365)          #@ ok salary_since
    r = r.assign(monthly=r["current_salary"] / 12, bonus_share=1 - r["current_salary"] / r["total_comp"])
    return r  #@row r: present={emp_id,name,dept_id,dept_name,location,location_dept,hire_date,remote,manager_id,salary_since,current_salary,total_comp,tenure_years,tenure_band,raise_pending,monthly,bonus_share} definite={emp_id,name,dept_id,dept_name,location,hire_date,remote,salary_since,current_salary,total_comp,tenure_years,tenure_band,raise_pending,monthly,bonus_share} closed


def dept_stats(r):
    st = r.groupby("dept_name").agg(headcount=("emp_id", "count"), avg_salary=("current_salary", "mean"), max_tenure=("tenure_years", "max"), remote_share=("remote", "mean"))
    #@row st: present={headcount,avg_salary,max_tenure,remote_share} definite={headcount,avg_salary,max_tenure,remote_share} absent={dept_name,emp_id} closed
    st = st.reset_index()
    st["payroll"] = st["headcount"] * st["avg_salary"]            #@ ok headcount ; ok avg_salary
    ncols = st.shape[1]
    #@value ncols: exactly={6}
    return st  #@row st: present={dept_name,headcount,avg_salary,max_tenure,remote_share,payroll} definite={dept_name,headcount,avg_salary,max_tenure,remote_share,payroll} closed


def stats_emp(st):
    return st["emp_id"]                                           #@ missing emp_id


def rank_in_dept(r):
    r = r.copy()
    r["salary_rank"] = r.groupby("dept_name")["current_salary"].rank(ascending=False, method="dense")
    r["dept_avg"] = r.groupby("dept_name")["current_salary"].transform("mean")
    r["vs_avg"] = r["current_salary"] - r["dept_avg"]                                 #@ ok dept_avg
    top = r[r["salary_rank"] == 1][["dept_name", "name", "current_salary", "vs_avg"]]
    #@row top: present={dept_name,name,current_salary,vs_avg} definite={dept_name,name,current_salary,vs_avg} absent={salary_rank,dept_avg} closed
    lines = []
    for t in top.itertuples(index=False):
        lines.append(f"{t.dept_name}: {t.name} ({t.current_salary:.0f})")           #@ ok dept_name ; ok name
        #@row t: present={dept_name,name,current_salary,vs_avg} definite={dept_name,name,current_salary,vs_avg} closed
    seniors = r.query("tenure_years > 5 and salary_rank <= 2")
    #@row seniors: present={emp_id,name,dept_id,dept_name,location,location_dept,hire_date,remote,manager_id,salary_since,current_salary,total_comp,tenure_years,tenure_band,raise_pending,monthly,bonus_share,salary_rank,dept_avg,vs_avg} definite={emp_id,name,dept_id,dept_name,location,hire_date,remote,salary_since,current_salary,total_comp,tenure_years,tenure_band,raise_pending,monthly,bonus_share,salary_rank,dept_avg,vs_avg} closed
    return lines


def make_reviews(e):
    ids = e["emp_id"].tolist()                                    #@ ok emp_id
    reviews = pd.DataFrame({"emp_id": ids + ids, "review_date": pd.to_datetime(["2021-12-31"] * len(ids) + ["2024-06-30"] * len(ids)), "rating": [3] * len(ids) + [4] * len(ids)})
    #@row reviews: present={emp_id,review_date,rating} definite={emp_id,review_date,rating} closed
    return reviews


def salary_at_reviews(reviews, s):
    left = reviews.sort_values("review_date")
    right = s.sort_values("effective_date")
    m = pd.merge_asof(left, right, left_on="review_date", right_on="effective_date", by="emp_id", direction="backward")
    #@row m: present={emp_id,review_date,rating,effective_date,salary,total_comp} definite={emp_id,review_date,rating,effective_date,salary,total_comp} closed
    m = m.drop(columns=["effective_date"]).rename(columns={"salary": "salary_at_review"})
    m["unpaid"] = m["salary_at_review"].isna()                    #@ ok salary_at_review
    per_rating = m.groupby("rating", as_index=False)["salary_at_review"].mean()
    #@row per_rating: present={rating,salary_at_review} definite={rating,salary_at_review} closed
    return m  #@row m: present={emp_id,review_date,rating,salary_at_review,total_comp,unpaid} definite={emp_id,review_date,rating,salary_at_review,total_comp,unpaid} absent={effective_date,salary} closed


def run(emp_text, dept_text, sal_text):
    e = load_employees(emp_text)
    try:
        initials_of(e)
    except KeyError:
        pass
    d = load_departments(dept_text)
    s = load_salaries(sal_text)
    latest = latest_salary(s)
    try:
        old_salary(latest)
    except KeyError:
        pass
    r = build_roster(e, d, latest)
    for probe in (office_of_dept, managers):
        try:
            probe(r)
        except KeyError:
            pass
    team_sizes(r)
    r = tenure_features(r, AS_OF)
    st = dept_stats(r)
    try:
        stats_emp(st)
    except KeyError:
        pass
    rank_in_dept(r)
    salary_at_reviews(make_reviews(e), s)
    return st


if __name__ == "__main__":
    run(EMPLOYEES_A, DEPARTMENTS_A, SALARIES_A)
    run(EMPLOYEES_B, DEPARTMENTS_B, SALARIES_B)
    run(EMPLOYEES_C, DEPARTMENTS_A, SALARIES_C)

#@schema read_csv@L66: always={emp_id,dept_id,first,last,hire_date} sometimes={manager_id} only={emp_id,dept_id,first,last,hire_date}
#@schema read_csv@L83: always={dept_id,dept_name,location} only={dept_id,dept_name,location}
#@schema read_csv@L92: always={emp_id,effective_date,salary} only={emp_id,effective_date,salary}

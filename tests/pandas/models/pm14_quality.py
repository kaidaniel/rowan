# Data-quality report over arbitrary frames: generic functions that never name a column of
# their own (null counts via isna().sum(), a dtype inventory from dtypes, dtype groups keyed by
# literal kinds, duplicate detection on an optional key, a rules dict mapping column -> check,
# per-column profiles as dicts with optional keys); the report is a dict of frames and dicts
# whose keys are what is claimed. Run on two CSV inputs and one frame built in code.
import io

import pandas as pd

PEOPLE_CSV = """id,name,email,age,country
1,Ann,ann@example.org,34,DE
2,Bob,,41,FR
3,Cy,cy@example.org,,USA
3,Cy,cy@example.org,,USA
4,Dee,dee-at-example.org,130,NL
"""

ORDERS_CSV = """order_id,customer,amount,placed
A1,1,20.5,2024-01-03
A2,2,,2024-01-04
A3,4,7.25,2024-01-09
"""

PEOPLE_RULES = {
    "email": lambda s: s.str.contains("@", na=False),
    "age": lambda s: s.between(0, 120),
    "country": lambda s: s.str.len() == 2,
}

ORDER_RULES = {
    "amount": lambda s: s.fillna(0) >= 0,
    "currency": lambda s: s.isin(["EUR", "USD"]),
}

STOCK = pd.DataFrame({"sku": ["x1", "x2", "x2"], "qty": [5, None, 3], "bin": ["A", "B", "B"]})
#@row STOCK: present={sku,qty,bin} definite={sku,qty,bin} closed


def null_report(df):
    counts = df.isna().sum()
    out = []
    for col, n in counts.items():
        out.append({"column": col, "nulls": int(n), "share": float(n) / len(df) if len(df) else 0.0})
    nulls = pd.DataFrame(out, columns=["column", "nulls", "share"])
    #@row nulls: present={column,nulls,share} definite={column,nulls,share} closed
    return nulls[nulls["nulls"] > 0]                            #@ ok nulls


def dtype_inventory(df):
    inv = df.dtypes.astype(str).rename("dtype").rename_axis("column").reset_index()
    #@row inv: present={column,dtype} definite={column,dtype} closed
    return inv.sort_values("column")                            #@ ok column


def type_groups(df):
    groups = {}
    for col, dt in df.dtypes.items():
        kind = "numeric" if pd.api.types.is_numeric_dtype(dt) else "datetime" if pd.api.types.is_datetime64_any_dtype(dt) else "text"
        groups.setdefault(kind, []).append(col)
    #@row groups: present={numeric,datetime,text} definite={} closed
    return groups


def duplicate_report(df, key=None):
    if key is not None:
        keys = df[key]
        dup = keys.duplicated(keep=False)
    else:
        dup = df.duplicated(keep=False)
    dups = df[dup]
    res = {"rows": int(dup.sum()), "key": key, "sample": dups.head(3).to_dict("records")}
    #@row res: present={rows,key,sample} definite={rows,key,sample} closed
    return res


def apply_rules(df, rules):
    results = []
    for col, check in rules.items():
        if col not in df.columns:
            results.append({"column": col, "status": "absent"})
            continue
        passed = check(df[col])                                 #@ ok email ; ok age ; ok country ; ok amount ; ok currency
        n_bad = int((~passed).sum())
        results.append({"column": col, "status": "ok" if n_bad == 0 else "failed", "failed": n_bad, "checked": len(passed)})
    table = pd.DataFrame(results)
    # (results has one record per rule; rowan does not know that the loop ran, so nothing is
    # definite: an empty list makes a frame without columns)
    #@todo row table: definite={column,status}
    #@row table: present={column,status,failed,checked} definite={} closed
    return table


def column_profile(df, col):
    s = df[col]
    prof = {"column": col, "dtype": str(s.dtype), "nulls": int(s.isna().sum()), "unique": int(s.nunique())}
    if pd.api.types.is_numeric_dtype(s):
        prof.update(min=float(s.min()), max=float(s.max()), mean=float(s.mean()))
    else:
        prof["top"] = s.mode().iloc[0] if s.notna().any() else None
    # (the row directive stood on its own line under the else branch, which checks that branch)
    return prof  #@row prof: present={column,dtype,nulls,unique,min,max,mean,top} definite={column,dtype,nulls,unique} closed


def quality_report(name, df, rules, key=None):
    report = {"dataset": name, "rows": len(df), "columns": len(df.columns)}
    report["nulls"] = null_report(df)
    report["dtypes"] = dtype_inventory(df)
    report["groups"] = type_groups(df)
    report["duplicates"] = duplicate_report(df, key)
    report["rules"] = apply_rules(df, rules)
    report["profiles"] = [column_profile(df, c) for c in df.columns]
    if report["duplicates"]["rows"]:                            #@ ok duplicates ; ok rows
        report["warning"] = "duplicates"
    # (as above: the directive moved from under the if branch to the return)
    return report  #@row report: present={dataset,rows,columns,nulls,dtypes,groups,duplicates,rules,profiles,warning} definite={dataset,rows,columns,nulls,dtypes,groups,duplicates,rules,profiles} closed


def warning_of(report):
    return report["warning"]                                    #@ possibly warning


def null_detail(report):
    return report["nulls"]["missing"]                           #@ ok nulls ; missing missing


def rule_failures(report):
    rules = report["rules"]                                     #@ ok rules
    if "failed" not in rules.columns:
        return 0
    return int(rules["failed"].fillna(0).sum())                 #@ ok failed


def profile_numbers(report):
    out = []
    for prof in report["profiles"]:
        if "mean" in prof:
            # (update() adds min, max and mean together: a relation rowan does not keep)
            #@todo ok max ; ok min
            out.append({"column": prof["column"], "mean": prof["mean"], "range": prof["max"] - prof["min"]})   #@ ok column ; ok mean ; sound max ; sound min
    return out


def first_top(report):
    return report["profiles"][0]["top"]                         #@ possibly top


def summary_frame(reports):
    rows = []
    for r in reports:
        rows.append({"dataset": r["dataset"], "rows": r["rows"], "null_cells": int(r["nulls"]["nulls"].sum()), "failed_rules": rule_failures(r), "dup_rows": r["duplicates"]["rows"]})   #@ ok dataset ; ok nulls ; ok duplicates
    summary = pd.DataFrame(rows).set_index("dataset")
    #@row summary: present={rows,null_cells,failed_rules,dup_rows} definite={rows,null_cells,failed_rules,dup_rows} absent={dataset} closed
    return summary


def check_report(report):
    try:
        warning_of(report)
    except KeyError:
        pass
    try:
        null_detail(report)
    except KeyError:
        pass
    try:
        first_top(report)
    except KeyError:
        pass
    nums = profile_numbers(report)
    return len(nums)


if __name__ == "__main__":
    people = pd.read_csv(io.StringIO(PEOPLE_CSV))
    orders = pd.read_csv(io.StringIO(ORDERS_CSV))
    reports = [
        quality_report("people", people, PEOPLE_RULES, key="id"),
        quality_report("orders", orders, ORDER_RULES, key="order_id"),
        quality_report("stock", STOCK, {"qty": lambda s: s.notna()}),
    ]
    for rep in reports:
        check_report(rep)
    summary = summary_frame(reports)
    dup = reports[0]["duplicates"]                              #@row dup: present={rows,key,sample} definite={rows,key,sample} closed
    sample = dup["sample"]
    first = sample[0]                                           #@row first: present={id} definite={id} open
    n_reports = len(reports)                                    #@value n_reports: exactly={3}

# (first written with sometimes={email,age,country} / {amount,currency}: the rules read a
# column only after `if col not in df.columns: continue`, so no input needs them)
#@schema read_csv@L174: always={id} only={id}
#@schema read_csv@L175: always={order_id} only={order_id}

# Inventory reconciliation: warehouse counts vs system stock; filter / reindex projections, outer merge with indicator=True and an optional overlapping column, discrepancy columns, crosstab, recounts applied with update (no new columns) and combine_first (union)
import io

import pandas as pd

KEYS = ["sku", "location"]

COUNTS_A = """sku,location,counted_qty,counted_at
 a-1 ,WH1,10,2024-03-01 09:00
A-2,WH1,0,2024-03-01 09:05
A-2,WH1,3,2024-03-01 10:00
B-7,WH2,5,2024-03-01 11:00
Z-9,WH2,2,2024-03-01 11:30
"""

SYSTEM_A = """sku,location,system_qty,unit_cost
A-1,WH1,12,2.5
A-2,WH1,3,4.0
B-7,WH2,5,10.0
C-3,WH2,7,1.0
"""

RECOUNT_A = """sku,location,counted_qty,recounted_by
A-1,WH1,12,sam
"""

# both sides carry a note column; the counts name who counted; the system reserves stock
COUNTS_B = """sku,location,counted_qty,counted_at,counter,note
A-1,WH1,12,2024-04-01 08:00,kim,ok
B-7,WH2,1,2024-04-01 08:30,lee,damaged box
"""

SYSTEM_B = """sku,location,system_qty,unit_cost,reserved,note
a-1,WH1,14,2.5,2,
B-7,WH2,4,10.0,0,audit
"""

RECOUNT_B = """sku,location,counted_qty,recounted_by,comment
B-7,WH2,4,ana,found 3 more
Q-1,WH9,1,ana,stray
"""

COUNTS_C = """sku,location,counted_qty,counted_at
A-1,WH3,1,2024-05-01 07:00
"""

SYSTEM_C = """sku,location,system_qty,unit_cost
A-1,WH3,1,2.5
"""

RECOUNT_C = """sku,location,counted_qty,recounted_by
A-1,WH3,1,
"""


def load_counts(text):
    c = pd.read_csv(io.StringIO(text))
    c["sku"] = c["sku"].str.strip().str.upper()                             #@ input sku
    c["location"] = c["location"].str.strip()                               #@ input location
    c["counted_at"] = pd.to_datetime(c["counted_at"])                       #@ input counted_at
    c["counted_qty"] = c["counted_qty"].astype(float)                       #@ input counted_qty
    c = c.sort_values("counted_at").drop_duplicates(KEYS, keep="last")
    c = c.filter(items=["sku", "location", "counted_qty", "counted_at", "note"])
    return c  #@row c: present={sku,location,counted_qty,counted_at,note} definite={sku,location,counted_qty,counted_at} absent={counter} closed


def load_system(text):
    s = pd.read_csv(io.StringIO(text))
    s["sku"] = s["sku"].str.upper()                                         #@ input sku
    s["location"] = s["location"].str.strip()                               #@ input location
    s["system_qty"] = s["system_qty"].astype(float)                         #@ input system_qty
    s["unit_cost"] = pd.to_numeric(s["unit_cost"])                          #@ input unit_cost
    if "reserved" in s.columns:
        s["system_qty"] = s["system_qty"] - s["reserved"].fillna(0)         #@ ok reserved
    s = s.reindex(columns=["sku", "location", "system_qty", "unit_cost", "note"])
    return s  #@row s: present={sku,location,system_qty,unit_cost,note} definite={sku,location,system_qty,unit_cost,note} absent={reserved} closed


def reconcile(c, s):
    m = c.merge(s, on=KEYS, how="outer", indicator=True)
    #@row m: present={sku,location,counted_qty,counted_at,system_qty,unit_cost,note,note_x,note_y,_merge} definite={sku,location,counted_qty,counted_at,system_qty,unit_cost,_merge} closed
    m["status"] = m["_merge"].map({"both": "matched", "left_only": "unexpected", "right_only": "not_counted"}).astype(str)  #@ ok _merge
    m = m.drop(columns=["_merge"])
    m["counted_qty"] = m["counted_qty"].fillna(0)                           #@ ok counted_qty
    m["system_qty"] = m["system_qty"].fillna(0)                             #@ ok system_qty
    m["diff"] = m["counted_qty"] - m["system_qty"]
    m["abs_diff"] = m["diff"].abs()
    m = m.assign(**{"value at risk": m["abs_diff"] * m["unit_cost"].fillna(0)})
    m["severity"] = pd.cut(m["abs_diff"], bins=[-1, 0, 2, float("inf")], labels=["none", "minor", "major"]).astype(str)
    return m  #@row m: present={sku,location,counted_qty,counted_at,system_qty,unit_cost,note,note_x,note_y,status,diff,abs_diff,"value at risk",severity} definite={sku,location,counted_qty,counted_at,system_qty,unit_cost,status,diff,abs_diff,"value at risk",severity} absent={_merge} closed


def merge_flag(m):
    return m["_merge"]                                                      #@ missing _merge


def counter_notes(m):
    return m["note_x"].dropna()                                             #@ possibly note_x


def system_notes(m):
    if "note" in m.columns:
        return m["note"].dropna()                                           #@ ok note
    # (note absent means the counts had a note too, so note_y exists: a relation between columns
    # rowan does not keep)
    return m["note_y"].dropna()                                             #@ sound note_y
    #@todo ok note_y


def location_summary(m):
    g = m.groupby("location", as_index=False).agg(items=("sku", "nunique"), net_diff=("diff", "sum"), at_risk=("value at risk", "sum"), worst=("abs_diff", "max"))
    #@row g: present={location,items,net_diff,at_risk,worst} definite={location,items,net_diff,at_risk,worst} closed
    g["flag"] = g["worst"] > 2                                              #@ ok worst
    ct = pd.crosstab(m["location"], m["status"])
    #@row ct: open
    ct = ct.reindex(columns=["matched", "unexpected", "not_counted"], fill_value=0)
    #@row ct: present={matched,unexpected,not_counted} definite={matched,unexpected,not_counted} closed
    both = g.merge(ct, left_on="location", right_index=True, how="left")
    #@row both: present={location,items,net_diff,at_risk,worst,flag,matched,unexpected,not_counted} definite={location,items,net_diff,at_risk,worst,flag,matched,unexpected,not_counted} closed
    return both


def load_recount(text):
    r = pd.read_csv(io.StringIO(text))
    r["sku"] = r["sku"].astype(str).str.upper()                             #@ input sku
    r["location"] = r["location"].astype(str)                               #@ input location
    r["counted_qty"] = r["counted_qty"].astype(float)                       #@ input counted_qty
    r["recounted_by"] = r["recounted_by"].fillna("unknown")                 #@ input recounted_by
    r = r.set_index(KEYS)
    return r  #@row r: definite={counted_qty,recounted_by} absent={sku,location} open


def apply_update(m, recount):
    upd = m.set_index(KEYS)
    #@row upd: present={counted_qty,counted_at,system_qty,unit_cost,note,note_x,note_y,status,diff,abs_diff,"value at risk",severity} definite={counted_qty,counted_at,system_qty,unit_cost,status,diff,abs_diff,"value at risk",severity} absent={sku,location} closed
    upd.update(recount)
    #@row upd: present={counted_qty,counted_at,system_qty,unit_cost,note,note_x,note_y,status,diff,abs_diff,"value at risk",severity} definite={counted_qty,counted_at,system_qty,unit_cost,status,diff,abs_diff,"value at risk",severity} absent={sku,location,recounted_by,comment} closed
    upd["diff"] = upd["counted_qty"] - upd["system_qty"]                    #@ ok counted_qty ; ok system_qty
    out = upd.reset_index()
    #@row out: present={sku,location,counted_qty,counted_at,system_qty,unit_cost,note,note_x,note_y,status,diff,abs_diff,"value at risk",severity} definite={sku,location,counted_qty,counted_at,system_qty,unit_cost,status,diff,abs_diff,"value at risk",severity} absent={recounted_by,index} closed
    return out


def recounted_by_after_update(m, recount):
    upd = m.set_index(KEYS)
    upd.update(recount)
    return upd["recounted_by"]                                              #@ missing recounted_by


def apply_combine(m, recount):
    combined = recount.combine_first(m.set_index(KEYS))
    #@row combined: definite={counted_qty,recounted_by,counted_at,system_qty,unit_cost,status,diff,abs_diff,"value at risk",severity} absent={sku,location} open
    combined = combined.reset_index()
    combined["recounted_by"] = combined["recounted_by"].fillna("-")         #@ ok recounted_by
    combined["sku"]                                                         #@ ok sku
    return combined


def exceptions_report(m):
    ex = m.query("status != 'matched' or abs_diff > 0")
    ex = ex.sort_values(["location", "abs_diff"], ascending=[True, False])
    out = ex[["sku", "location", "status", "diff", "value at risk"]].rename(columns={"value at risk": "value_at_risk"})
    #@row out: present={sku,location,status,diff,value_at_risk} definite={sku,location,status,diff,value_at_risk} absent={"value at risk",severity} closed
    width = len(out.columns)
    #@value width: exactly={5}
    return [f"{rec['sku']}@{rec['location']}: {rec['value_at_risk']:.2f}" for rec in out.to_dict("records")]


def spaced_name(out):
    return out["value at risk"]                                             #@ missing "value at risk"


def run(counts_text, system_text, recount_text):
    m = reconcile(load_counts(counts_text), load_system(system_text))
    for probe in (merge_flag, counter_notes):
        try:
            probe(m)
        except KeyError:
            pass
    system_notes(m)
    location_summary(m)
    recount = load_recount(recount_text)
    apply_update(m, recount)
    try:
        recounted_by_after_update(m, recount)
    except KeyError:
        pass
    apply_combine(m, recount)
    exceptions_report(m)
    try:
        spaced_name(m[["sku", "value at risk"]].rename(columns={"value at risk": "value_at_risk"}))
    except KeyError:
        pass
    return m


if __name__ == "__main__":
    run(COUNTS_A, SYSTEM_A, RECOUNT_A)
    run(COUNTS_B, SYSTEM_B, RECOUNT_B)
    run(COUNTS_C, SYSTEM_C, RECOUNT_C)

#@schema read_csv@L57: always={sku,location,counted_at,counted_qty} only={sku,location,counted_at,counted_qty}
#@schema read_csv@L68: always={sku,location,system_qty,unit_cost} only={sku,location,system_qty,unit_cost}
#@schema read_csv@L124: always={sku,location,counted_qty,recounted_by} only={sku,location,counted_qty,recounted_by}

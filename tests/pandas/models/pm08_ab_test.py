"""A/B test analysis: assignments + conversions (merge how="left" with indicator=True),
groupby(["variant", "segment"], as_index=False) with an agg dict built per input, pivot /
pivot_table to wide, uplift columns keyed by the variant names, describe(); run on three
datasets (segment column or not, coupon column or not, different variant names)."""
import io

import pandas as pd

ASSIGN_1 = """user_id,variant,segment,signup_day
1,a,new,2024-02-01
2,b,new,2024-02-01
3,a,returning,2024-02-02
4,b,returning,2024-02-02
5,a,new,2024-02-03
6,b,returning,2024-02-03
7,a,returning,2024-02-04
8,b,new,2024-02-04
"""

ASSIGN_2 = """user_id,variant,country
21,control,DE
22,test,DE
23,control,FR
24,test,FR
25,test,FR
"""

CONV_1 = """user_id,converted,revenue,coupon
1,1,20.0,
2,1,35.0,SPRING
4,1,12.5,SPRING
6,0,0.0,
8,1,40.0,
"""

CONV_2 = """user_id,converted,revenue
3,1,15.0
5,1,22.0
22,1,30.0
23,0,0.0
25,1,18.0
"""


def load_assignments(text):
    assign = pd.read_csv(io.StringIO(text))
    assign["variant"] = assign["variant"].str.strip().str.upper()        #@ input variant
    if "segment" not in assign.columns:
        assign["segment"] = "all"
    assign["user_id"] = assign["user_id"].astype(int)                   #@ input user_id
    core = assign[["user_id", "variant", "segment"]]                    #@ ok user_id ; ok variant ; ok segment
    #@row core: present={user_id,variant,segment} definite={user_id,variant,segment} closed
    return core


def load_conversions(text):
    conv = pd.read_csv(io.StringIO(text))
    conv["user_id"] = conv["user_id"].astype(int)                       #@ input user_id
    conv["converted"] = conv["converted"].astype(int)                   #@ input converted
    conv["revenue"] = conv["revenue"].clip(lower=0)                     #@ input revenue
    cols = ["user_id", "converted", "revenue"]
    if "coupon" in conv.columns:
        conv["used_coupon"] = conv["coupon"].notna()                    #@ ok coupon
        cols = cols + ["used_coupon"]
    conv = conv[cols]
    #@row conv: present={user_id,converted,revenue,used_coupon} definite={user_id,converted,revenue} absent={coupon} closed
    return conv


def join_outcomes(core, conv):
    merged = pd.merge(core, conv, on="user_id", how="left", indicator=True)
    #@row merged: present={user_id,variant,segment,converted,revenue,used_coupon,_merge} definite={user_id,variant,segment,converted,revenue,_merge} closed
    merged["matched"] = merged["_merge"] == "both"                      #@ ok _merge
    merged = merged.drop(columns="_merge")
    merged["converted"] = merged["converted"].fillna(0).astype(int)     #@ ok converted
    merged["revenue"] = merged["revenue"].fillna(0.0)                   #@ ok revenue
    if "used_coupon" in merged.columns:
        merged["used_coupon"] = merged["used_coupon"].eq(True)          #@ ok used_coupon
    n = len(merged)  #@row merged: present={user_id,variant,segment,converted,revenue,used_coupon,matched} definite={user_id,variant,segment,converted,revenue,matched} absent={_merge} closed
    return merged


def summarise(merged):
    aggs = {
        "users": ("user_id", "nunique"),
        "conversions": ("converted", "sum"),
        "revenue": ("revenue", "sum"),
    }
    if "used_coupon" in merged.columns:
        aggs["coupon_rate"] = ("used_coupon", "mean")
    summary = merged.groupby(["variant", "segment"], as_index=False).agg(**aggs)
    #@row summary: present={variant,segment,users,conversions,revenue,coupon_rate} definite={variant,segment,users,conversions,revenue} absent={user_id,converted} closed
    summary["cr"] = summary["conversions"] / summary["users"]           #@ ok conversions ; ok users
    summary["arpu"] = summary["revenue"] / summary["users"]             #@ ok revenue
    summary = summary.sort_values(["segment", "variant"]).reset_index(drop=True)
    #@row summary: present={variant,segment,users,conversions,revenue,coupon_rate,cr,arpu} definite={variant,segment,users,conversions,revenue,cr,arpu} closed
    return summary


def probe_users(summary):
    # user ids were aggregated into the users count
    return summary["user_id"]                                           #@ missing user_id


def probe_coupon(summary):
    # only datasets with a coupon column get the rate
    return summary["coupon_rate"]                                       #@ possibly coupon_rate


def to_wide(summary, control, treatment):
    wide = summary.pivot(index="segment", columns="variant", values="cr")  #@ ok segment
    #@row wide: present={} definite={} open
    wide = wide.fillna(0.0)
    wide["uplift"] = wide[treatment] - wide[control]
    wide["rel_uplift"] = wide["uplift"] / wide[control].where(wide[control] > 0)  #@ ok uplift
    wide.columns.name = None
    wide = wide.reset_index()
    #@row wide: present={segment,uplift,rel_uplift,A,B,CONTROL,TEST} definite={segment,uplift,rel_uplift} open
    return wide


def revenue_view(merged):
    rev = merged.pivot_table(index="segment", columns="variant", values="revenue", aggfunc="mean", fill_value=0)
    #@row rev: present={} definite={} open
    rev = rev.add_suffix("_arpu")
    #@row rev: present={} definite={} absent={segment,revenue} open
    desc = merged.groupby("variant")["revenue"].describe()              #@ ok variant ; ok revenue
    #@row desc: present={count,mean,std,min,"25%","50%","75%",max} definite={count,mean,std,min,"25%","50%","75%",max} closed
    spread = desc["max"] - desc["min"]                                  #@ ok max ; ok min
    both = merged[["revenue", "converted"]].describe()
    #@row both: present={revenue,converted} definite={revenue,converted} closed
    return rev, desc


def probe_median(desc):
    # describe() names the median "50%"
    return desc["median"]                                               #@ missing median


def decide(wide, control, treatment):
    decisions = []
    for rec in wide.to_dict("records"):
        seg = rec["segment"]                                            #@ ok segment
        lift = rec["uplift"]                                            #@ ok uplift
        winner = treatment if lift > 0 else control
        #@value winner: exactly={"A","B","CONTROL","TEST"}
        decision = "ship" if lift > 0.05 else "hold"
        #@value decision: exactly={"ship","hold"}
        decisions.append((seg, winner, decision))
    return decisions


def analyse(assign_text, conv_text, control, treatment):
    core = load_assignments(assign_text)
    conv = load_conversions(conv_text)
    merged = join_outcomes(core, conv)
    summary = summarise(merged)
    wide = to_wide(summary, control, treatment)
    rev, desc = revenue_view(merged)
    for probe, arg in ((probe_users, summary), (probe_coupon, summary), (probe_median, desc)):
        try:
            probe(arg)
        except KeyError:
            pass
    return decide(wide, control, treatment)


if __name__ == "__main__":
    print(analyse(ASSIGN_1, CONV_1, "A", "B"))
    print(analyse(ASSIGN_2, CONV_2, "CONTROL", "TEST"))
    print(analyse(ASSIGN_1, CONV_2, "A", "B"))
#@schema read_csv@L46: always={user_id,variant} only={user_id,variant}
#@schema read_csv@L57: always={user_id,converted,revenue} only={user_id,converted,revenue}

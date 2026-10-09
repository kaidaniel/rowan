"""Customer churn features: events + subscriptions -> per-customer features (groupby named agg
built from a dict, days-since via .dt, crosstab of event kinds), merges of feature tables,
fillna with a dict, model-ready selection (select_dtypes, filter(like=/items=)); run on two
datasets (optional device / region columns, different event kinds)."""
import io

import pandas as pd

AS_OF = pd.Timestamp("2024-07-01")

EVENTS_1 = """customer_id,ts,event,amount
1,2024-05-02 10:00,login,
1,2024-05-03 11:30,Purchase,19.99
1,2024-06-20 09:15,login,
2,2024-03-14 18:00,login,
2,2024-03-15 18:05,ticket,
3,2024-06-29 07:45,purchase,5.50
3,2024-06-30 08:00,login,
"""

EVENTS_2 = """customer_id,ts,event,amount,device
10,2024-01-05 12:00,login,,mobile
10,2024-01-06 12:30,purchase,42.00,mobile
11,2024-06-01 21:00,login,,desktop
11,2024-06-25 21:10,purchase,8.00,mobile
12,2024-02-11 06:00,login,,desktop
"""

SUBS_1 = """customer_id,plan,start_date,end_date,monthly_fee,region
1,pro,2023-01-10,,29.0,EU
2,basic,2023-06-01,2024-04-01,9.0,US
3,basic,2024-05-20,,9.0,EU
4,pro,2022-11-11,2024-01-15,29.0,APAC
"""

SUBS_2 = """customer_id,plan,start_date,end_date,monthly_fee,referrer
10,basic,2023-12-01,2024-03-01,9.0,ads
11,team,2024-01-01,,99.0,
12,basic,2023-08-08,2024-02-20,9.0,friend
"""


def load_events(text):
    ev = pd.read_csv(io.StringIO(text), parse_dates=["ts"])
    ev["customer_id"] = ev["customer_id"].astype(int)                  #@ input customer_id
    ev["event"] = ev["event"].str.lower().str.strip()                   #@ input event
    ev["amount"] = ev["amount"].fillna(0.0)                             #@ input amount
    ev["weekend"] = ev["ts"].dt.dayofweek >= 5                          #@ input ts
    if "device" in ev.columns:
        ev["mobile"] = ev["device"].eq("mobile").astype(float)          #@ ok device
    count = len(ev)  #@row ev: present={customer_id,event,amount,ts,weekend,device,mobile} definite={customer_id,event,amount,ts,weekend} open
    return ev


def event_features(ev):
    ev["is_login"] = (ev["event"] == "login").astype(int)               #@ ok event
    ev["is_ticket"] = (ev["event"] == "ticket").astype(int)
    spec = {
        "n_events": ("event", "size"),
        "logins": ("is_login", "sum"),
        "tickets": ("is_ticket", "sum"),
        "spend": ("amount", "sum"),
        "last_seen": ("ts", "max"),
    }
    if "mobile" in ev.columns:
        spec["mobile_share"] = ("mobile", "mean")
    n_specs = len(spec)  #@value n_specs: exactly={5,6}
    #@row spec: present={n_events,logins,tickets,spend,last_seen,mobile_share} definite={n_events,logins,tickets,spend,last_seen} closed
    feats = ev.groupby("customer_id").agg(**spec)
    #@row feats: present={n_events,logins,tickets,spend,last_seen,mobile_share} definite={n_events,logins,tickets,spend,last_seen} absent={customer_id,event} closed
    feats["days_since_seen"] = (AS_OF - feats["last_seen"]).dt.days    #@ ok last_seen
    feats = feats.drop(columns=["last_seen"])
    feats["spend_per_event"] = feats["spend"] / feats["n_events"]       #@ ok spend ; ok n_events
    feats = feats.reset_index()
    #@row feats: present={customer_id,n_events,logins,tickets,spend,mobile_share,days_since_seen,spend_per_event} definite={customer_id,n_events,logins,tickets,spend,days_since_seen,spend_per_event} absent={last_seen} closed
    return feats


def probe_last_seen(feats):
    return feats["last_seen"]                                           #@ missing last_seen


def event_kinds(ev):
    kinds = pd.crosstab(ev["customer_id"], ev["event"]).add_prefix("ev_")  #@ ok customer_id ; ok event
    #@row kinds: present={} definite={} open
    kinds = kinds.reset_index()
    #@row kinds: present={customer_id} definite={customer_id} open
    return kinds


def load_subscriptions(text):
    subs = pd.read_csv(io.StringIO(text), parse_dates=["start_date", "end_date"])
    subs["tenure_days"] = (AS_OF - subs["start_date"]).dt.days         #@ input start_date
    subs["churned"] = subs["end_date"].notna()                          #@ input end_date
    if "region" not in subs.columns:
        subs["region"] = "unknown"
    subs["monthly_fee"] = subs["monthly_fee"].astype(float)             #@ input monthly_fee
    subs["customer_id"] = subs["customer_id"].astype(int)              #@ input customer_id
    subs["plan"] = subs["plan"].str.strip().str.lower()                 #@ input plan
    base = subs[["customer_id", "plan", "region", "monthly_fee", "tenure_days", "churned"]]  #@ ok customer_id ; ok plan ; ok region ; ok tenure_days
    #@row base: present={customer_id,plan,region,monthly_fee,tenure_days,churned} definite={customer_id,plan,region,monthly_fee,tenure_days,churned} closed
    return base


def build_table(base, feats, kinds):
    table = base.merge(feats, on="customer_id", how="left")
    #@row table: present={customer_id,plan,region,monthly_fee,tenure_days,churned,n_events,logins,tickets,spend,mobile_share,days_since_seen,spend_per_event} definite={customer_id,plan,region,monthly_fee,tenure_days,churned,n_events,logins,tickets,spend,days_since_seen,spend_per_event} closed
    fills = {"n_events": 0, "logins": 0, "tickets": 0, "spend": 0.0, "days_since_seen": 999, "spend_per_event": 0.0}
    table = table.fillna(fills)
    table["fee_x_tenure"] = table["monthly_fee"] * table["tenure_days"] / 30  #@ ok monthly_fee ; ok tenure_days
    table = table.merge(kinds, on="customer_id", how="left")
    #@row table: present={customer_id,plan,region,monthly_fee,tenure_days,churned,n_events,logins,tickets,spend,mobile_share,days_since_seen,spend_per_event,fee_x_tenure} definite={customer_id,plan,region,monthly_fee,tenure_days,churned,n_events,logins,tickets,spend,days_since_seen,spend_per_event,fee_x_tenure} open
    table = pd.get_dummies(table, columns=["plan"], prefix="plan", dtype=int)
    #@row table: present={customer_id,region,monthly_fee,tenure_days,churned,n_events,logins,tickets,spend,mobile_share,days_since_seen,spend_per_event,fee_x_tenure} definite={customer_id,region,monthly_fee,tenure_days,churned,n_events,logins,tickets,spend,days_since_seen,spend_per_event,fee_x_tenure} absent={plan} open
    return table


def probe_mobile(table):
    # only the dataset with a device column has the share
    return table["mobile_share"]                                        #@ possibly mobile_share


def probe_plan(table):
    # one-hot encoded into plan_* columns
    return table["plan"]                                                #@ missing plan


def model_inputs(table):
    target = table["churned"].astype(int)                               #@ ok churned
    numeric = table.select_dtypes(include="number")
    #@row numeric: definite_within={customer_id,monthly_fee,tenure_days,n_events,logins,tickets,spend,mobile_share,days_since_seen,spend_per_event,fee_x_tenure} absent={churned} open
    X = numeric.drop(columns=["customer_id"])
    #@row X: absent={customer_id,churned} open
    activity = table.filter(like="ev_")
    #@row activity: present={} definite={} absent={customer_id,churned,n_events,days_since_seen} open
    core = table.filter(items=["tenure_days", "spend", "tickets"])
    #@row core: present={tenure_days,spend,tickets} definite={tenure_days,spend,tickets} closed
    core = core.rename(columns=str.upper)
    #@row core: present={TENURE_DAYS,SPEND,TICKETS} definite={TENURE_DAYS,SPEND,TICKETS} absent={tenure_days} closed
    return X, target, activity, core


def risk_report(table):
    flagged = []
    records = table[["customer_id", "days_since_seen", "tickets"]].to_dict("records")
    for rec in records:
        status = "risk" if rec["days_since_seen"] > 60 or rec["tickets"] > 0 else "ok"  #@ ok days_since_seen ; ok tickets
        #@value status: exactly={"risk","ok"}
        if status == "risk":
            flagged.append(rec["customer_id"])                          #@ ok customer_id
    return flagged, records


def probe_region(records):
    # the report records were projected to three columns
    return records[0]["monthly_fee"]                                    #@ missing monthly_fee


def run(events_text, subs_text):
    ev = load_events(events_text)
    feats = event_features(ev)
    kinds = event_kinds(ev)
    base = load_subscriptions(subs_text)
    table = build_table(base, feats, kinds)
    X, y, activity, core = model_inputs(table)
    for probe in (probe_last_seen, probe_mobile, probe_plan):
        try:
            probe(feats if probe is probe_last_seen else table)
        except KeyError:
            pass
    flagged, records = risk_report(table)
    try:
        probe_region(records)
    except KeyError:
        pass
    return X.shape, list(activity.columns), flagged


if __name__ == "__main__":
    for events_text, subs_text in [(EVENTS_1, SUBS_1), (EVENTS_2, SUBS_2)]:
        print(run(events_text, subs_text))
#@schema read_csv@L44: always={customer_id,event,amount,ts} only={customer_id,event,amount,ts}
#@schema read_csv@L92: always={customer_id,plan,start_date,end_date,monthly_fee} only={customer_id,plan,start_date,end_date,monthly_fee}

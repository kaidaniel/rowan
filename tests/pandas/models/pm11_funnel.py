# E-commerce funnel: click events with step names -> pivot of step counts per session
# (reindexed to the known steps), conversion rates between consecutive steps (dict keys from
# an f-string over zip), a funnel table built from record dicts, melt back to long form,
# signup-week cohorts via dt.to_period merged with buyers, a device x step crosstab with
# margins; optional `coupon` / `device` / `amount` columns differ between the two datasets.
import io

import pandas as pd

EVENTS_A = """session_id,user_id,step,ts,device,coupon,amount
s1,u1,view,2024-01-01 10:00,mobile,,
s1,u1,Cart,2024-01-01 10:02,mobile,,
s1,u1,checkout,2024-01-01 10:05,mobile,SAVE5,
s1,u1,purchase,2024-01-01 10:06,mobile,SAVE5,40.0
s2,u2,view,2024-01-02 09:00,desktop,,
s2,u2,cart,2024-01-02 09:03,desktop,,
s3,u3,view,2024-01-09 12:00,mobile,,
s3,u3,view,2024-01-09 12:01,mobile,,
s4,u1,view,2024-01-10 08:00,desktop,,
s4,u1, cart ,2024-01-10 08:01,desktop,,
s4,u1,checkout,2024-01-10 08:02,desktop,,
s4,u1,purchase,2024-01-10 08:04,desktop,,25.5
s5,u4,wishlist,2024-01-11 18:00,tablet,,
"""

# no device / coupon / amount columns, no purchase at all, an extra `referrer` column
EVENTS_B = """session_id,user_id,step,ts,referrer
t1,v1,view,2024-02-01 10:00,ads
t1,v1,cart,2024-02-01 10:01,ads
t2,v2,view,2024-02-03 11:00,seo
t2,v2,checkout,2024-02-03 11:09,seo
t3,v3,VIEW,2024-02-04 15:00,
"""

USERS_A = """user_id,signup_date,country
u1,2023-12-28,DE
u2,2024-01-02,FR
u3,2024-01-08,DE
u4,2024-01-09,US
"""

USERS_B = """user_id,signup_date,country,plan
v1,2024-01-29,US,free
v2,2024-01-30,US,pro
v3,2024-02-02,CA,free
"""

STEPS = ["view", "cart", "checkout", "purchase"]


def load_events(text):
    ev = pd.read_csv(io.StringIO(text))
    ev["step"] = ev["step"].str.strip().str.lower()            #@ input step
    ev["ts"] = pd.to_datetime(ev["ts"])                         #@ input ts
    ev = ev[ev["step"].isin(STEPS)]                             #@ ok step
    sess = ev["session_id"]                                     #@ input session_id
    ev = ev.drop_duplicates(subset=["session_id", "step"])
    #@row ev: present={step,ts,session_id} definite={step,ts,session_id} open
    return ev.sort_values(["session_id", "ts"])


def load_users(text):
    users = pd.read_csv(io.StringIO(text))
    users["signup_date"] = pd.to_datetime(users["signup_date"])   #@ input signup_date
    return users                                                 #@row users: present={signup_date} definite={signup_date} open


def step_counts(ev):
    counts = ev.pivot_table(index="session_id", columns="step", values="user_id", aggfunc="count", fill_value=0)
    counts = counts.reindex(columns=STEPS, fill_value=0)
    #@row counts: present={view,cart,checkout,purchase} definite={view,cart,checkout,purchase} closed
    counts["depth"] = (counts[STEPS] > 0).sum(axis=1)
    return counts                                               #@row counts: present={view,cart,checkout,purchase,depth} definite={view,cart,checkout,purchase,depth} closed


def conversion(counts):
    reached = (counts[STEPS] > 0).sum()
    rates = {}
    for a, b in zip(STEPS, STEPS[1:]):
        rates[f"{a}->{b}"] = float(reached[b] / reached[a]) if reached[a] else 0.0
    n_pairs = len(rates)                                        #@value n_pairs: exactly={3}
    #@row rates: present={"view->cart","cart->checkout","checkout->purchase"} definite={"view->cart","cart->checkout","checkout->purchase"} closed
    overall = rates["view->cart"] * rates["cart->checkout"]     #@ ok "view->cart" ; ok "cart->checkout"
    return rates, overall


def funnel_table(counts):
    rows = []
    for i, step in enumerate(STEPS):
        rows.append({"step": step, "order": i, "sessions": int((counts[step] > 0).sum())})
    funnel = pd.DataFrame(rows)                                 #@row funnel: present={step,order,sessions} definite={step,order,sessions} closed
    funnel["drop_off"] = 1 - funnel["sessions"] / funnel["sessions"].shift(1)
    funnel = funnel.set_index("step")                           #@row funnel: present={order,sessions,drop_off} definite={order,sessions,drop_off} absent={step} closed
    for step, row in funnel.iterrows():
        row["sessions"]                                         #@ ok sessions
    return funnel


def funnel_by_name(funnel):
    return funnel["step"]                                       #@ missing step


def long_form(counts):
    long = counts.reset_index().melt(id_vars=["session_id"], value_vars=STEPS, var_name="step", value_name="n")
    #@row long: present={session_id,step,n} definite={session_id,step,n} closed
    per_step = long.groupby("step", as_index=False)["n"].sum()
    #@row per_step: present={step,n} definite={step,n} closed
    per_step["share"] = per_step["n"] / per_step["n"].sum()    #@ ok n
    return per_step


def cohorts(users, ev):
    users = users.copy()
    users["cohort"] = users["signup_date"].dt.to_period("W").astype(str)   #@ ok signup_date
    buyers = ev.loc[ev["step"] == "purchase", ["user_id"]].drop_duplicates()
    buyers["bought"] = 1                                        #@row buyers: present={user_id,bought} definite={user_id,bought} closed
    merged = users.merge(buyers, on="user_id", how="left")      #@ input user_id
    merged["bought"] = merged["bought"].fillna(0).astype(int)   #@ ok bought
    #@row merged: present={user_id,signup_date,cohort,bought} definite={user_id,signup_date,cohort,bought} open
    table = merged.groupby("cohort").agg(users=("user_id", "nunique"), buyers=("bought", "sum")).reset_index()
    #@row table: present={cohort,users,buyers} definite={cohort,users,buyers} closed
    table["rate"] = table["buyers"] / table["users"]
    return table                                                #@row table: present={cohort,users,buyers,rate} definite={cohort,users,buyers,rate} closed


def cohort_revenue(table):
    return table["revenue"].sum()                               #@ missing revenue


def device_by_step(ev):
    tab = pd.crosstab(ev["device"], ev["step"], margins=True)   #@ input device ; ok step
    #@row tab: present={All} definite={All} open
    return tab["All"]                                           #@ ok All


def coupon_share(ev):
    if "coupon" not in ev.columns:
        return 0.0
    return float(ev["coupon"].notna().mean())                   #@ ok coupon


def purchase_value(ev):
    if "amount" in ev.columns:
        ev = ev.assign(value=ev["amount"].fillna(0.0))          #@ ok amount
    n_cols = len(ev.columns)                                    #@row ev: present={step,ts,session_id,user_id,device,coupon,amount,value} definite={step,ts,session_id,user_id} open
    return float(ev["value"].sum())                             #@ possibly value


def run(events_text, users_text):
    ev = load_events(events_text)
    users = load_users(users_text)
    counts = step_counts(ev)
    rates, overall = conversion(counts)
    funnel = funnel_table(counts)
    try:
        funnel_by_name(funnel)
    except KeyError:
        pass
    per_step = long_form(counts)
    table = cohorts(users, ev)
    try:
        cohort_revenue(table)
    except KeyError:
        pass
    if "device" in ev.columns:
        device_by_step(ev)
    share = coupon_share(ev)
    try:
        value = purchase_value(ev)
    except KeyError:
        value = None
    summary = {"sessions": len(counts), "overall": overall, "coupon_share": share, "value": value}
    #@row summary: present={sessions,overall,coupon_share,value} definite={sessions,overall,coupon_share,value} closed
    return summary, rates, per_step


if __name__ == "__main__":
    res_a = run(EVENTS_A, USERS_A)
    res_b = run(EVENTS_B, USERS_B)
    report = {"a": res_a[0], "b": res_b[0]}                     #@row report: present={a,b} definite={a,b} closed
    first = res_a[1]["view->cart"]                              #@ ok "view->cart"

#@schema read_csv@L52: always={step,ts,session_id,user_id} sometimes={device,coupon,amount} only={step,ts,session_id,user_id,amount,device,coupon}
#@schema read_csv@L63: always={user_id,signup_date} only={user_id,signup_date}

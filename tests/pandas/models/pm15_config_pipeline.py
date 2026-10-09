# Config-driven pipeline: a dict config lists required input columns, defaults for optional
# ones, the columns to keep (or None = every input column), renames, derived columns
# (op, left, right), an optional flag column and aggregations (named tuples, or generated per
# numeric column, so data-dependent names); the pipeline applies it generically. The driver
# runs the sales config on two inputs (with / without the optional `discount`) and the tickets
# config (all columns kept, generated aggregations) on a third: precise where the config is
# literal, sound (open) where column names come from the data.
import io

import pandas as pd

SALES_A = """id,region,product,units,unit_price,discount,channel
1,north,widget,3,9.5,1.0,web
2,north,gadget,1,120.0,0.0,shop
3,south,widget,10,9.5,5.0,web
4,south,gizmo,2,45.0,,web
"""

SALES_B = """id,region,product,units,unit_price
10,east,widget,4,10.0
11,east,gadget,2,110.0
12,west,gizmo,1,50.0
"""

TICKETS = """id,team,opened,closed,priority,reopens
100,core,2024-03-01 09:00,2024-03-01 17:00,high,0
101,core,2024-03-02 10:00,2024-03-04 10:00,low,1
102,web,2024-03-02 11:30,2024-03-02 12:00,high,0
103,web,2024-03-03 08:00,2024-03-05 20:00,,2
"""

CONFIG_SALES = {
    "name": "sales",
    "required": ["id", "region", "product", "units", "unit_price"],
    "defaults": {"discount": 0.0},
    "columns": ["id", "region", "product", "units", "unit_price", "discount"],
    "rename": {"unit_price": "price"},
    "derived": [("revenue", "mul", "units", "price"), ("net", "sub", "revenue", "discount")],
    "flag": ("net", 100.0),
    "group_by": ["region"],
    "aggregations": {"revenue": ("revenue", "sum"), "units": ("units", "sum"), "avg_net": ("net", "mean"), "orders": ("id", "count")},
}

CONFIG_TICKETS = {
    "name": "tickets",
    "required": ["team", "opened", "closed"],
    "defaults": {"priority": "low"},
    "columns": None,
    "rename": {"opened": "opened_at", "closed": "closed_at"},
    "derived": [("hours", "hours_between", "closed_at", "opened_at")],
    "group_by": ["team", "priority"],
    "aggregations": "numeric_means",
}


def hours_between(a, b):
    return (pd.to_datetime(a) - pd.to_datetime(b)).dt.total_seconds() / 3600


OPS = {
    "mul": lambda a, b: a * b,
    "sub": lambda a, b: a - b,
    "hours_between": hours_between,
}


def validate(df, cfg):
    for col in cfg["required"]:
        df[col]                                                  #@ sound id ; sound region ; sound units ; sound team ; sound opened
    return df


def fill_defaults(df, cfg):
    df = df.copy()
    for col, default in cfg.get("defaults", {}).items():
        if col not in df.columns:
            df[col] = default
    n = len(df.columns)                                          #@row df: present={id,region,product,units,unit_price,discount,team,opened,closed,priority} definite={} open
    return df


def select(df, cfg):
    cols = cfg["columns"] if cfg["columns"] is not None else list(df.columns)
    return df[cols]


def rename(df, cfg):
    return df.rename(columns=cfg.get("rename", {}))


def derive(df, cfg):
    df = df.copy()
    for name, op, left, right in cfg.get("derived", []):
        df[name] = OPS[op](df[left], df[right])                  #@ sound units ; sound price ; sound revenue ; sound discount ; sound closed_at ; sound opened_at
    flag = cfg.get("flag")
    if flag is not None:
        col, threshold = flag
        df["high"] = df[col] > threshold                         #@ ok net
    return df


def aggregate(df, cfg):
    keys = list(cfg["group_by"])
    if cfg["aggregations"] == "numeric_means":
        spec = {f"{c}_mean": (c, "mean") for c in df.select_dtypes("number").columns if c not in keys}
    else:
        spec = {out: tuple(src) for out, src in cfg["aggregations"].items()}
    out = df.groupby(keys, as_index=False).agg(**spec)
    return out


def run_pipeline(raw, cfg):
    df = validate(raw, cfg)
    df = fill_defaults(df, cfg)
    sel = select(df, cfg)
    ren = rename(sel, cfg)
    der = derive(ren, cfg)
    agg = aggregate(der, cfg)
    out = agg.add_prefix(cfg["name"] + "_")
    result = {"detail": der, "summary": agg, "export": out}
    #@row result: present={detail,summary,export} definite={detail,summary,export} closed
    return result


def old_names(det, cfg):
    for old in cfg["rename"]:
        det[old]                                                 #@ missing unit_price


def high_count(det):
    return int(det["high"].sum())                                #@ ok high


def high_of_open(det):
    return det["high"]                                           #@ input high


def first_key(export, cfg):
    return export[cfg["name"] + "_" + cfg["group_by"][0]]        #@ ok sales_region ; ok tickets_team


def export_records(export, cfg):
    prefix = cfg["name"] + "_"
    rows = []
    for rec in export.to_dict("records"):
        rows.append({k[len(prefix):]: v for k, v in rec.items()})
    return rows


if __name__ == "__main__":
    sales_a = pd.read_csv(io.StringIO(SALES_A))
    sales_b = pd.read_csv(io.StringIO(SALES_B))
    tickets = pd.read_csv(io.StringIO(TICKETS))
    ra = run_pipeline(sales_a, CONFIG_SALES)
    rb = run_pipeline(sales_b, CONFIG_SALES)
    rt = run_pipeline(tickets, CONFIG_TICKETS)
    det_a = ra["detail"]                                         #@row det_a: present={id,region,product,units,price,discount,revenue,net,high} definite={id,region,product,units,price,discount,revenue,net,high} absent={unit_price,channel} closed
    det_b = rb["detail"]                                         #@row det_b: present={id,region,product,units,price,discount,revenue,net,high} definite={id,region,product,units,price,discount,revenue,net,high} absent={unit_price} closed
    sum_b = rb["summary"]                                        #@row sum_b: present={region,revenue,units,avg_net,orders} definite={region,revenue,units,avg_net,orders} absent={net,price} closed
    exp_a = ra["export"]                                         #@row exp_a: present={sales_region,sales_revenue,sales_units,sales_avg_net,sales_orders} definite={sales_region,sales_revenue,sales_units,sales_avg_net,sales_orders} absent={region,revenue} closed
    n_out = len(exp_a.columns)                                   #@value n_out: exactly={5}
    det_t = rt["detail"]                                         #@row det_t: present={team,opened_at,closed_at,priority,hours} definite={team,opened_at,closed_at,priority,hours} absent={opened,closed} open
    sum_t = rt["summary"]                                        #@row sum_t: present={team,priority} definite={team,priority} open
    exp_t = rt["export"]                                         #@row exp_t: present={tickets_team,tickets_priority} definite={tickets_team,tickets_priority} absent={team,priority} open
    total = exp_a["sales_revenue"].sum()                         #@ ok sales_revenue
    hours = exp_t["tickets_hours_mean"]                          #@ sound tickets_hours_mean
    try:
        exp_a["region"]                                          #@ missing region
    except KeyError:
        pass
    for res, cfg in [(ra, CONFIG_SALES), (rb, CONFIG_SALES), (rt, CONFIG_TICKETS)]:
        first_key(res["export"], cfg)
        if cfg is CONFIG_TICKETS:
            try:
                high_of_open(res["detail"])
            except KeyError:
                pass
            continue
        try:
            high_count(res["detail"])
        except KeyError:
            pass
    for res in (ra, rb):
        try:
            old_names(res["detail"], CONFIG_SALES)
        except KeyError:
            pass
    recs = export_records(exp_a, CONFIG_SALES)
    rec = recs[0]                                                #@row rec: present={region,revenue,units,avg_net,orders} definite={region,revenue,units,avg_net,orders} closed
    prefix = CONFIG_SALES["name"] + "_"                          #@value prefix: exactly={"sales_"}

#@schema read_csv@L151: always={id,region,product,units,unit_price} sometimes={discount} only={id,region,product,units,unit_price,discount}
#@schema read_csv@L152: always={id,region,product,units,unit_price} sometimes={discount} only={id,region,product,units,unit_price,discount}
#@schema read_csv@L153: always={team,opened,closed} sometimes={priority} only={team,opened,closed,priority}

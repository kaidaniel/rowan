# Weather station aggregation: one CSV per station, concatenated with keys= (station codes from
# the dict) and the key level turned into a column; station metadata flattened with
# json_normalize from nested literal records (dotted column names), renamed and projected;
# merge; groupby station + month with named aggregations (tuples and pd.NamedAgg);
# unstack / reindex by the station codes / stack back; optional `precip` / `wind` columns and
# metadata fields differ between the two station sets.
import io

import pandas as pd

SET_A = {
    "OSL": """date,tmax,tmin,precip
2024-01-30,-2.0,-9.5,1.2
2024-01-31,0.5,-6.0,
2024-02-01,1.5,-3.0,4.0
2024-02-02,-1.0,-7.5,0.0
""",
    "BGO": """date,tmax,tmin,precip,wind
2024-01-31,6.0,1.0,12.5,14.0
2024-02-01,7.5,2.5,20.0,22.5
2024-02-02,5.0,0.5,8.0,9.0
""",
    "TRD": """date,tmax,tmin
2024-01-31,-4.0,-12.0
2024-02-01,-3.5,-10.0
""",
}

SET_B = {
    "OSL": """date,tmax,tmin,snow
2024-03-30,8.0,-1.0,0
2024-03-31,9.5,0.5,0
2024-04-01,11.0,2.0,0
""",
    "KRS": """date,tmax,tmin
2024-03-31,12.0,4.0
2024-04-01,13.5,5.5
""",
}

META_A = [
    {"id": "OSL", "name": "Oslo Blindern", "location": {"lat": 59.94, "lon": 10.72, "elev": 94}, "owner": {"org": "MET"}},
    {"id": "BGO", "name": "Bergen Florida", "location": {"lat": 60.38, "lon": 5.33, "elev": 12}, "owner": {"org": "MET"}},
    {"id": "TRD", "name": "Trondheim Voll", "location": {"lat": 63.41, "lon": 10.45}, "owner": {"org": "NTNU"}},
]

META_B = [
    {"id": "OSL", "name": "Oslo Blindern", "location": {"lat": 59.94, "lon": 10.72}, "active": True},
    {"id": "KRS", "name": "Kjevik", "location": {"lat": 58.2, "lon": 8.08}, "active": False},
]


def load_station(text):
    df = pd.read_csv(io.StringIO(text))
    df["date"] = pd.to_datetime(df["date"])                      #@ input date
    df["tmean"] = (df["tmax"] + df["tmin"]) / 2                  #@ input tmax ; input tmin
    df = df.query("tmax >= tmin")                                #@row df: present={date,tmax,tmin,tmean} definite={date,tmax,tmin,tmean} open
    return df.sort_values("date")                                #@ ok date


def load_all(texts):
    frames = [load_station(text) for text in texts.values()]
    obs = pd.concat(frames, keys=list(texts), names=["station", "row"])
    obs = obs.reset_index(level="station").reset_index(drop=True)
    # (first written absent={row}: the frames are open inputs, which could hold a `row` column)
    #@row obs: present={station,date,tmax,tmin,tmean} definite={station,date,tmax,tmin,tmean} open
    if "precip" in obs.columns:
        obs["precip"] = obs["precip"].fillna(0.0)                #@ ok precip
    else:
        obs["precip"] = 0.0
    return obs                                                   #@row obs: present={station,date,tmax,tmin,tmean,precip} definite={station,date,tmax,tmin,tmean,precip} open


def load_meta(records):
    meta = pd.json_normalize(records)
    #@row meta: present={id,name,"location.lat","location.lon","location.elev","owner.org",active} definite={id,name,"location.lat","location.lon"} closed
    meta = meta.rename(columns={"location.lat": "lat", "location.lon": "lon", "location.elev": "elev", "owner.org": "org"})
    #@row meta: present={id,name,lat,lon,elev,org,active} definite={id,name,lat,lon} absent={"location.lat","location.lon","location.elev","owner.org"} closed
    if "elev" not in meta.columns:
        meta["elev"] = 0.0
    meta = meta[["id", "name", "lat", "lon", "elev"]]           #@ ok id ; ok lat ; ok elev
    return meta                                                  #@row meta: present={id,name,lat,lon,elev} definite={id,name,lat,lon,elev} closed


def org_of(meta):
    return meta["org"]                                           #@ missing org


def monthly(obs):
    obs = obs.assign(month=obs["date"].dt.to_period("M").astype(str), trange=obs["tmax"] - obs["tmin"])
    agg = obs.groupby(["station", "month"]).agg(tmax_mean=pd.NamedAgg(column="tmax", aggfunc="mean"), tmin_min=("tmin", "min"), rain=("precip", "sum"), wettest=("precip", "max"), days=("date", "count"), spread=("trange", "mean"))
    #@row agg: present={tmax_mean,tmin_min,rain,wettest,days,spread} definite={tmax_mean,tmin_min,rain,wettest,days,spread} absent={station,month,tmax,precip} closed
    agg["dry"] = agg["rain"] == 0                                #@ ok rain
    flat = agg.reset_index()
    #@row flat: present={station,month,tmax_mean,tmin_min,rain,wettest,days,spread,dry} definite={station,month,tmax_mean,tmin_min,rain,wettest,days,spread,dry} closed
    return flat


def monthly_tmax(flat):
    return flat["tmax"]                                          #@ missing tmax


def wide_by_station(flat, codes):
    wide = flat.set_index(["month", "station"])["tmax_mean"].unstack("station")
    #@row wide: present={} definite={} open
    wide = wide.reindex(columns=codes)
    #@row wide: present={OSL,BGO,TRD,KRS} definite={OSL} closed
    osl = wide["OSL"]                                            #@ ok OSL
    long = wide.stack().reset_index()
    long.columns = ["month", "station", "tmax_mean"]
    #@row long: present={month,station,tmax_mean} definite={month,station,tmax_mean} closed
    rain = flat.pivot(index="station", columns="month", values="rain")
    #@row rain: present={} definite={} open
    return wide, long, rain


def attach_meta(flat, meta):
    out = flat.merge(meta.rename(columns={"id": "station"}), on="station", how="left")
    #@row out: present={station,month,tmax_mean,tmin_min,rain,wettest,days,spread,dry,name,lat,lon,elev} definite={station,month,tmax_mean,tmin_min,rain,wettest,days,spread,dry,name,lat,lon,elev} absent={id} closed
    out["tmax_sea"] = out["tmax_mean"] + 0.0065 * out["elev"]   #@ ok elev
    out["id"]                                                    #@ missing id
    return out


def attach_meta_safe(flat, meta):
    try:
        return attach_meta(flat, meta)
    except KeyError:
        return flat.merge(meta, left_on="station", right_on="id").assign(tmax_sea=0.0)


def gusts(obs):
    if "wind" in obs.columns:
        obs = obs.assign(gust=obs["wind"] * 1.5)                 #@ ok wind
    return float(obs["gust"].max())                              #@ input gust


def summary_records(out):
    rows = []
    for rec in out.to_dict("records"):
        rows.append({"station": rec["station"], "month": rec["month"], "label": f"{rec['name']} ({rec['station']})", "warm": rec["tmax_sea"] > 5})   #@ ok station ; ok month ; ok name ; ok tmax_sea
    return rows


def process(texts, records):
    obs = load_all(texts)
    meta = load_meta(records)
    try:
        org_of(meta)
    except KeyError:
        pass
    flat = monthly(obs)
    try:
        monthly_tmax(flat)
    except KeyError:
        pass
    wide, long, rain = wide_by_station(flat, list(texts))
    out = attach_meta_safe(flat, meta)                           #@row out: present={station,month,tmax_mean,tmin_min,rain,wettest,days,spread,dry,name,lat,lon,elev,id,tmax_sea} definite={station,month,tmax_mean,tmin_min,rain,wettest,days,spread,dry,name,lat,lon,elev,id,tmax_sea} closed
    try:
        g = gusts(obs)
    except KeyError:
        g = None
    rows = summary_records(out)
    first = rows[0]                                              #@row first: present={station,month,label,warm} definite={station,month,label,warm} closed
    return {"stations": len(texts), "months": long["month"].nunique(), "gust": g, "rows": len(rows)}


if __name__ == "__main__":
    res_a = process(SET_A, META_A)
    n_a = res_a["stations"]                                      #@value n_a: exactly={3}
    res_b = process(SET_B, META_B)
    n_b = res_b["stations"]                                      #@value n_b: exactly={2}
    keys = sorted(res_a)                                         #@row res_a: present={stations,months,gust,rows} definite={stations,months,gust,rows} closed

# (first written with sometimes={precip,wind}: their reads are behind `in` tests; gust as
# `possibly` above: the inputs are open)
#@schema read_csv@L54: always={date,tmax,tmin} only={date,tmax,tmin}

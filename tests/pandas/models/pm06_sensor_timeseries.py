"""Time series pipeline: sensor readings on a DatetimeIndex (resample with an agg dict and named
aggregations, rolling / shift / diff features, interpolate, asfreq, between_time, join of daily
weather), run on three sites whose optional columns (pressure, battery, firmware) differ."""
import io

import pandas as pd

SITE_A = """timestamp,sensor,temp,humidity,pressure
2024-03-01 00:00,s1,20.5,40,1012
2024-03-01 06:00,s1,,42,1011
2024-03-01 12:00,s1,23.0,38,
2024-03-01 18:00,s1,21.5,41,1012
2024-03-02 00:00,s1,19.0,45,1013
2024-03-02 06:00,s1,18.5,,1014
2024-03-02 12:00,s1,22.0,39,1012
2024-03-02 18:00,s1,20.0,43,1011
2024-03-03 00:00,s1,18.0,47,1010
2024-03-03 12:00,s1,21.0,44,1009
"""

SITE_B = """timestamp,sensor,temp,humidity,battery
2024-03-01 00:00,b7,15.0,60,98
2024-03-01 12:00,b7,17.5,58,97
2024-03-02 00:00,b7,,61,96
2024-03-02 12:00,b7,18.0,55,95
2024-03-03 00:00,b7,14.5,63,94
2024-03-03 12:00,b7,16.0,59,93
"""

SITE_C = """timestamp,sensor,temp,humidity,pressure,battery,firmware
2024-03-01 03:00,c2,11.0,70,1001,91,v2
2024-03-01 09:00,c2,12.5,68,1000,90,v2
2024-03-01 15:00,c2,14.0,,999,90,v2
2024-03-02 03:00,c2,10.5,72,1002,89,v3
2024-03-02 15:00,c2,13.0,66,1003,88,v3
2024-03-03 03:00,c2,9.5,75,,87,v3
2024-03-03 15:00,c2,12.0,69,1004,86,v3
"""

WEATHER = """date,outside_temp,rain_mm,wind_kmh
2024-03-01,8.0,0.0,12
2024-03-02,6.5,3.2,30
2024-03-03,7.0,,18
"""

WEATHER_SHORT = """date,outside_temp,rain_mm,wind_kmh
2024-03-01,5.0,1.5,8
2024-03-02,4.0,0.0,10
"""


def load_readings(text):
    raw = pd.read_csv(io.StringIO(text))
    raw["timestamp"] = pd.to_datetime(raw["timestamp"])                  #@ input timestamp
    readings = raw.set_index("timestamp").sort_index()
    #@row readings: present={} definite={} absent={timestamp} open
    return readings


def clean(readings):
    out = readings.copy()
    out["temp"] = out["temp"].interpolate(method="time")                 #@ input temp
    out["humidity"] = out["humidity"].interpolate(method="time").round(1)  #@ input humidity
    if "pressure" in out.columns:
        out["pressure"] = out["pressure"].ffill()                       #@ ok pressure
    if "firmware" in out.columns:
        out = out.drop(columns=["firmware"])
    n_rows = len(out)  #@row out: present={temp,humidity,pressure} definite={temp,humidity} absent={timestamp,firmware} open
    return out


def daily_summary(readings):
    spec = {"temp": "mean", "humidity": "max"}
    if "pressure" in readings.columns:
        spec["pressure"] = "mean"
    n_specs = len(spec)  #@row spec: present={temp,humidity,pressure} definite={temp,humidity} closed
    #@value n_specs: exactly={2,3}
    daily = readings.resample("D").agg(spec)
    #@row daily: present={temp,humidity,pressure} definite={temp,humidity} closed
    ranges = readings.resample("D").agg(t_min=("temp", "min"), t_max=("temp", "max"))
    #@row ranges: present={t_min,t_max} definite={t_min,t_max} closed
    daily["temp_range"] = ranges["t_max"] - ranges["t_min"]             #@ ok t_max ; ok t_min
    daily = daily.rename(columns={"temp": "temp_mean", "humidity": "humidity_max"})
    days = len(daily)  #@row daily: present={temp_mean,humidity_max,pressure,temp_range} definite={temp_mean,humidity_max,temp_range} absent={temp,humidity} closed
    return daily


def probe_daily_temp(daily):
    # renamed to temp_mean above
    return daily["temp"]                                                #@ missing temp


def add_features(cleaned):
    feats = cleaned[["temp", "humidity"]].copy()
    #@row feats: present={temp,humidity} definite={temp,humidity} closed
    feats["temp_roll3"] = feats["temp"].rolling(3, min_periods=1).mean()  #@ ok temp
    feats["temp_lag1"] = feats["temp"].shift(1)
    feats["humidity_diff"] = feats["humidity"].diff()                   #@ ok humidity
    if "battery" in cleaned.columns:
        feats["battery_low"] = cleaned["battery"] < 95                  #@ ok battery
    smooth = feats[["temp", "humidity"]].rolling("12h").mean().add_suffix("_12h")
    #@row smooth: present={temp_12h,humidity_12h} definite={temp_12h,humidity_12h} closed
    lagged = feats[["temp", "humidity"]].shift(2).add_suffix("_lag2")
    #@row lagged: present={temp_lag2,humidity_lag2} definite={temp_lag2,humidity_lag2} absent={temp,humidity} closed
    wide = pd.concat([feats, smooth], axis=1).join(lagged)
    #@row wide: present={temp,humidity,temp_roll3,temp_lag1,humidity_diff,battery_low,temp_12h,humidity_12h,temp_lag2,humidity_lag2} definite={temp,humidity,temp_roll3,temp_lag1,humidity_diff,temp_12h,humidity_12h,temp_lag2,humidity_lag2} closed
    daytime = wide.between_time("06:00", "18:00")
    #@row daytime: present={temp,humidity,temp_roll3,temp_lag1,humidity_diff,battery_low,temp_12h,humidity_12h,temp_lag2,humidity_lag2} definite={temp,humidity,temp_roll3,temp_lag1,humidity_diff,temp_12h,humidity_12h,temp_lag2,humidity_lag2} closed
    regular = cleaned[["temp"]].asfreq("6h")
    #@row regular: present={temp} definite={temp} closed
    regular["temp"] = regular["temp"].bfill()                           #@ ok temp
    return feats


def probe_battery(feats):
    # only sites with a battery column get the flag
    return feats["battery_low"]                                         #@ possibly battery_low


def load_weather(text):
    weather = pd.read_csv(io.StringIO(text), parse_dates=["date"])
    weather["rain_mm"] = weather["rain_mm"].fillna(0.0)                 #@ input rain_mm
    weather = weather.sort_values("date").set_index("date")
    #@row weather: present={rain_mm} definite={rain_mm} absent={date} open
    return weather


def attach_weather(daily, weather):
    joined = daily.join(weather, how="left")
    #@row joined: present={temp_mean,humidity_max,pressure,temp_range,rain_mm} definite={temp_mean,humidity_max,temp_range,rain_mm} absent={date} open
    joined["feels_delta"] = joined["temp_mean"] - joined["outside_temp"]  #@ ok temp_mean ; input outside_temp
    joined["rainy"] = joined["rain_mm"] > 1.0                           #@ ok rain_mm
    joined = joined.fillna({"outside_temp": joined["outside_temp"].mean(), "rain_mm": 0.0})  #@ ok outside_temp
    report = joined.reset_index().rename(columns={"timestamp": "day"})
    #@row report: present={day,temp_mean,humidity_max,pressure,temp_range,rain_mm,outside_temp,feels_delta,rainy} definite={day,temp_mean,humidity_max,temp_range,rain_mm,outside_temp,feels_delta,rainy} absent={timestamp,date} open
    labels = []
    for rec in report.to_dict("records"):
        day = rec["day"]                                                #@ ok day
        label = "wet" if rec["rainy"] else "dry"                        #@ ok rainy
        #@value label: exactly={"wet","dry"}
        labels.append(label)
    return report, labels


def sensor_stats(readings):
    stats = readings.groupby("sensor").agg(n=("temp", "count"), last_temp=("temp", "last"), peak_hum=("humidity", "max"))
    #@row stats: present={n,last_temp,peak_hum} definite={n,last_temp,peak_hum} absent={sensor} closed
    stats = stats.reset_index()
    #@row stats: present={sensor,n,last_temp,peak_hum} definite={sensor,n,last_temp,peak_hum} closed
    first = stats.iloc[0]
    first["peak_hum"]                                                   #@ ok peak_hum
    stats["avg_temp"] = stats["sensor"].map(readings.groupby("sensor")["temp"].mean())  #@ ok sensor
    return stats


def run_site(text, weather_text):
    readings = load_readings(text)
    mode = "with_pressure" if "pressure" in readings.columns else "basic"
    #@value mode: exactly={"with_pressure","basic"}
    cleaned = clean(readings)
    daily = daily_summary(cleaned)
    feats = add_features(cleaned)
    weather = load_weather(weather_text)
    report, labels = attach_weather(daily, weather)
    stats = sensor_stats(readings)
    try:
        probe_daily_temp(daily)
    except KeyError:
        pass
    try:
        probe_battery(feats)
    except KeyError:
        pass
    return report, stats, mode


if __name__ == "__main__":
    for site, wtext in [(SITE_A, WEATHER), (SITE_B, WEATHER_SHORT), (SITE_C, WEATHER)]:
        report, stats, mode = run_site(site, wtext)
        print(mode, report.shape, list(stats.columns))
#@schema read_csv@L53: always={timestamp,sensor,temp,humidity} only={timestamp,sensor,temp,humidity}
#@schema read_csv@L121: always={date,rain_mm,outside_temp} only={date,rain_mm,outside_temp}

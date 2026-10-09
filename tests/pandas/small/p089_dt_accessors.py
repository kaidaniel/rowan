# Series.dt: scalar accessors feed new columns; isocalendar() and timedelta components are frames with fixed columns
import pandas as pd


def calendar(df):
    ts = pd.to_datetime(df["when"])                      #@ ok when
    out = df.assign(year=ts.dt.year, month=ts.dt.month)  #@row out: present={when,amount,region,year,month} definite={when,year,month} closed
    iso = ts.dt.isocalendar()                            #@row iso: present={year,week,day} definite={year,week,day} closed
    parts = pd.DataFrame({"y": ts.dt.year, "dow": ts.dt.dayofweek})   #@row parts: present={y,dow} definite={y,dow} closed
    comps = (ts - ts.min()).dt.components                #@row comps: present={days,hours,minutes,seconds,milliseconds,microseconds,nanoseconds} definite={days,hours,minutes,seconds,milliseconds,microseconds,nanoseconds} closed
    iso["week"]                                          #@ ok week
    return iso["month"]                                  #@ missing month


def amounts_by_month(df):
    ts = pd.to_datetime(df["when"])
    out = df.assign(month=ts.dt.month)
    return out["amount"].groupby(out["month"]).sum()     #@ ok amount ; missing amount


if __name__ == "__main__":
    caught = []  # keeps each raised exception alive
    f1 = pd.DataFrame({"when": ["2024-01-02", "2024-03-04"], "amount": [1.0, 2.0]})
    f2 = pd.DataFrame({"when": ["2023-12-31"], "region": ["n"]})
    for frame in (f1, f2):
        try:
            calendar(frame)
        except KeyError as e:
            caught.append(e)
    amounts_by_month(f1)
    try:
        amounts_by_month(f2)
    except KeyError as e:
        caught.append(e)

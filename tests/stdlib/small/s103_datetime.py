# datetime / date / timedelta on known values: parsing, arithmetic, comparisons, formatting, date keys; now() is unknown
from datetime import date, datetime, timedelta, timezone


def duration(ev):
    start = datetime.fromisoformat(ev["start"])
    end = datetime.fromisoformat(ev["end"])
    return (end - start).total_seconds() / 60
#@schema param duration.ev: always={start,end} only={start,end}


def by_day(events):
    out = {}
    for ev in events:
        day = datetime.fromisoformat(ev["start"]).date().isoformat()
        out.setdefault(day, []).append(ev["name"])
    return out


def label(d):
    return d.strftime("%a %d %b %Y")


m = duration({"start": "2024-03-01T09:00:00", "end": "2024-03-01T10:30:00"})   #@value m: exactly={90.0}
days = by_day([{"start": "2024-03-01T09:00", "name": "a"}, {"start": "2024-03-02T08:00", "name": "b"}])
#@row days: present={"2024-03-01","2024-03-02"} definite={"2024-03-01","2024-03-02"} closed
d = date(2024, 2, 28) + timedelta(days=2)
ds = str(d)                                          #@value ds: exactly={"2024-03-01"}
wd = d.weekday()                                     #@value wd: exactly={4}
lb = label(d)                                        #@value lb: exactly={"Fri 01 Mar 2024"}
dt = datetime(2024, 1, 2, 3, 4, 5)
iso = dt.isoformat()                                 #@value iso: exactly={"2024-01-02T03:04:05"}
sp = str(dt)                                         #@value sp: exactly={"2024-01-02 03:04:05"}
later = dt + timedelta(hours=25, minutes=1)
lh = later.hour                                      #@value lh: exactly={4}
ld = later.day                                       #@value ld: exactly={3}
gap = later - dt
gs = str(gap).replace(",", ";")                      #@value gs: exactly={"1 day; 1:01:00"}
earlier = dt < later                                 #@value earlier: exactly={True}
p = datetime.strptime("05/06/2023 14:30", "%d/%m/%Y %H:%M")
pm = p.month                                         #@value pm: exactly={6}
aware = datetime(2024, 1, 1, tzinfo=timezone.utc)
ai = aware.isoformat()                               #@value ai: exactly={"2024-01-01T00:00:00+00:00"}
neg = str(timedelta(seconds=-1)).replace(",", ";")   #@value neg: exactly={"-1 day; 23:59:59"}
r = repr(date(2024, 5, 6))                           #@value r: exactly={"datetime.date(2024, 5, 6)"}
now = datetime.now()
ny = now.year                                        #@value ny: unknown
try:
    date(2023, 2, 29)
except ValueError:
    bad = True
#@value bad: exactly={True}

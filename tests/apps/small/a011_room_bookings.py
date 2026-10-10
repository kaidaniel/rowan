# Room bookings: CSV text read with csv.DictReader over io.StringIO and JSON from os.environ, both through one normaliser that drops / adds keys and returns early on rejects, dataclasses with a datetime property, defaultdict / Counter summaries, csv.DictWriter output read back
"""Hypothesis: normalise() returns None (early) when `nights` is not digits, else the row with
`nights` an int, `start` a date and `promo` only when it was non-empty: for literal rows the
key set is decided per row. Bookings built from the literal CSV are exact (guest titled, end =
start + nights, promo None / "SPRING"); asdict() of a booking has the dataclass fields exactly,
the export row swaps tags / promo for `end`, and the text read back has exactly the writer's
fieldnames. JSON records need `room`, `guest` and `start` only when their `nights` is digits
(sometimes); `nights` and `promo` are read with .get and never required."""
import csv
import io
import json
import os
from collections import Counter, defaultdict
from dataclasses import asdict, dataclass, field
from datetime import date, timedelta
from typing import Optional

SAMPLE = "room,guest,start,nights,promo\nA1,ann,2024-03-01,2,\nB2,bo,2024-03-02,1,SPRING\nA1,cy,2024-03-05,x,\n"


@dataclass
class Booking:
    room: str
    guest: str
    start: date
    nights: int
    promo: Optional[str] = None
    tags: list = field(default_factory=list)

    @property
    def end(self):
        return self.start + timedelta(days=self.nights)


def normalise(row):
    rec = {k: (v.strip() if isinstance(v, str) else v) for k, v in row.items()}
    nights = str(rec.get("nights", ""))
    if not nights.isdigit():
        return None
    rec["nights"] = int(nights)
    if not rec.get("promo"):
        rec.pop("promo", None)
    rec["start"] = date.fromisoformat(rec["start"])    #@ ok start ; input start
    return rec


def build(rec):
    b = Booking(room=rec["room"], guest=rec["guest"].title(), start=rec["start"], nights=rec["nights"], promo=rec.get("promo"))
    if b.nights >= 3:
        b.tags.append("long")
    return b


def load_csv(text):
    out, rejects = [], 0
    for row in csv.DictReader(io.StringIO(text)):
        rec = normalise(row)
        if rec is None:
            rejects += 1
            continue
        out.append(build(rec))
    return out, rejects


def load_json(items):
    return [build(rec) for rec in map(normalise, items) if rec is not None]


def occupancy(bookings):
    nights = defaultdict(int)
    guests = Counter()
    for b in bookings:
        nights[b.room] += b.nights
        guests[b.guest] += 1
    return dict(nights), guests


def export(bookings):
    buf = io.StringIO()
    w = csv.DictWriter(buf, fieldnames=["room", "guest", "start", "end", "nights"])
    w.writeheader()
    for b in bookings:
        row = asdict(b)
        #@row row: present={room,guest,start,nights,promo,tags} definite={room,guest,start,nights,promo,tags} closed
        row["end"] = b.end.isoformat()
        del row["tags"]
        row.pop("promo")
        w.writerow(row)
    return buf.getvalue()


if __name__ == "__main__":
    bs, rej = load_csv(SAMPLE)
    nb = len(bs)                                 #@value nb: exactly={2}
    nrej = rej                                   #@value nrej: exactly={1}
    b0 = bs[0]
    #@row b0: present={room,guest,start,nights,promo,tags} definite={room,guest,start,nights,promo,tags} closed
    g0 = b0.guest                                #@value g0: exactly={Ann}
    e0 = b0.end.isoformat()                      #@value e0: exactly={"2024-03-03"}
    p0 = b0.promo                                #@value p0: exactly={None}
    p1 = bs[1].promo                             #@value p1: exactly={SPRING}
    n0 = normalise({"room": "C", "guest": "x", "start": "2024-01-01", "nights": "4", "promo": ""})
    #@row n0: present={room,guest,start,nights} definite={room,guest,start,nights} absent={promo} closed
    r_none = normalise({"nights": "two", "room": "D"})   #@value r_none: exactly={None}
    nights, guests = occupancy(bs)
    #@row nights: present={A1,B2} definite={A1,B2} closed
    a1 = nights["A1"]                            #@value a1: exactly={2}
    text = export(bs)
    back = list(csv.DictReader(io.StringIO(text)))
    h0 = back[0]
    #@row h0: present={room,guest,start,end,nights} definite={room,guest,start,end,nights} closed
    end1 = back[1]["end"]                        #@value end1: exactly={"2024-03-03"}

    items = json.loads(os.environ.get("BOOKINGS_JSON", '[{"room": "C3", "guest": "dee", "start": "2024-04-01", "nights": 3}, {"room": "C3", "guest": "eve", "start": "2024-04-04", "nights": "n/a"}]'))
    #@schema json.loads@L114['*']: sometimes={room,guest,start} only={room,guest,start}
    try:
        more = load_json(items)
    except KeyError:
        more = []
    nights2, _ = occupancy(more)
    export(more)

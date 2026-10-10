# Expense pipeline: csv.DictReader over literal text (and text from os.environ), re named groups, date.fromisoformat / datetime.strptime, Decimal sums, defaultdict / Counter / itertools.groupby / sorted and max with key=
import csv
import io
import os
import re
from collections import Counter, defaultdict
from datetime import date, datetime
from decimal import Decimal
from itertools import groupby

LEDGER = """date,account,amount,memo
2024-03-01,food,12.50,INV-1001 lunch
2024-03-01,travel,40.00,TRIP-77 taxi
2024-03-15,Food,7.25,coffee
2024-04-02,travel,120.10,TRIP-78 train
2024-04-20,office,15.00,INV-1002 paper
"""

LEGACY = """when;who;sum
01/05/2024;food;3.30
02/05/2024;office;9.70
"""

MEMO = re.compile(r"(?P<kind>[A-Z]+)-(?P<num>\d+)\s*(?P<text>.*)")


def parse_memo(memo):
    m = MEMO.match(memo)
    if m is None:
        return {"kind": "NONE", "text": memo.strip()}
    info = m.groupdict()
    #@row info: present={kind,num,text} definite={kind,num,text} closed
    info["num"] = int(info["num"])               #@ ok num
    return info


def normalize(raw):
    day = date.fromisoformat(raw["date"])
    entry = {"day": day, "account": raw["account"].lower(), "amount": Decimal(raw["amount"])}
    entry.update(parse_memo(raw.get("memo") or ""))
    #@row entry: present={day,account,amount,kind,num,text} definite={day,account,amount,kind,text} closed
    return entry
#@schema param normalize.raw: always={date,account,amount} only={date,account,amount}


def read_ledger(text):
    return [normalize(raw) for raw in csv.DictReader(io.StringIO(text))]


def read_legacy(text):
    out = []
    for raw in csv.DictReader(io.StringIO(text), delimiter=";"):
        when = datetime.strptime(raw["when"], "%d/%m/%Y")   #@ ok when
        out.append(normalize({"date": when.date().isoformat(), "account": raw["who"], "amount": raw["sum"]}))
    return out


def totals_by_account(entries):
    sums = defaultdict(Decimal)
    for e in entries:
        sums[e["account"]] += e["amount"]
    return {k: str(v) for k, v in sums.items()}


def monthly(entries):
    ordered = sorted(entries, key=lambda e: e["day"])
    out = []
    for month, grp in groupby(ordered, key=lambda e: e["day"].strftime("%Y-%m")):
        items = list(grp)
        row = {"month": month, "count": len(items), "total": str(sum((e["amount"] for e in items), Decimal("0")))}
        #@row row: present={month,count,total} definite={month,count,total} closed
        out.append(row)
    return out


def kind_counts(entries):
    c = Counter(e["kind"] for e in entries)
    return tuple(c.most_common())


def build_report(entries):
    report = {"accounts": totals_by_account(entries), "months": monthly(entries), "kinds": kind_counts(entries)}
    refs = [e for e in entries if "num" in e]
    if refs:
        report["refs"] = tuple(sorted(e["num"] for e in refs))
    _ = 0  #@row report: present={accounts,months,kinds,refs} definite={accounts,months,kinds} closed
    return report


if __name__ == "__main__":
    ledger = read_ledger(LEDGER)
    legacy = read_legacy(LEGACY)
    entries = ledger + legacy
    coffee = ledger[2]
    #@row coffee: present={day,account,amount,kind,text} definite={day,account,amount,kind,text} absent={num} closed
    acct = coffee["account"]                     #@value acct: exactly={food}
    l0 = legacy[0]["day"].isoformat()            #@value l0: exactly={"2024-05-01"}
    rep = build_report(entries)
    #@row rep: present={accounts,months,kinds,refs} definite={accounts,months,kinds,refs} closed
    accounts = rep["accounts"]
    #@row accounts: present={food,travel,office} definite={food,travel,office} closed
    food = accounts["food"]                      #@value food: exactly={"23.05"}
    travel = accounts["travel"]                  #@value travel: exactly={"160.10"}
    months = tuple(m["month"] for m in rep["months"])   #@value months: exactly={("2024-03","2024-04","2024-05")}
    may = rep["months"][2]["total"]              #@value may: exactly={"13.00"}
    kinds = rep["kinds"]                         #@value kinds: exactly={(("NONE",3),("INV",2),("TRIP",2))}
    refs = rep["refs"]                           #@value refs: exactly={(77,78,1001,1002)}
    top = max(entries, key=lambda e: e["amount"])["text"]   #@value top: exactly={train}
    extra = read_ledger(os.environ.get("LEDGER_EXTRA", ""))
    rep2 = build_report(legacy + extra)
    #@row rep2: present={accounts,months,kinds,refs} definite={accounts,months,kinds} closed

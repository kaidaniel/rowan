# a log report: lines parsed with a compiled pattern of named groups (groupdict), key=value fields via
# finditer, namedtuple entries, Counter / defaultdict / deque / OrderedDict aggregation, sorted with a
# key= callback, re.sub with a function masking e-mails, and a string.Template report

import re
import string
from collections import Counter, OrderedDict, defaultdict, deque, namedtuple

LINE = re.compile(r"(?P<ts>\d{2}:\d{2}:\d{2}) (?P<level>[A-Z]+) \[(?P<svc>\w+)\] (?P<msg>[^|]*)(?:\|(?P<extra>.*))?")
KV = re.compile(r"(?P<k>\w+)=(?P<v>\S+)")
EMAIL = re.compile(r"(?P<user>[\w.]+)@(?P<domain>[\w.]+)")
REPORT = string.Template("$total lines: $errors errors / $warnings warnings - slowest=$slowest")

Entry = namedtuple("Entry", "ts level svc msg fields")


def parse_fields(extra):
    if not extra:
        return {}
    return {m["k"]: m["v"] for m in KV.finditer(extra)}


def parse_line(line):
    m = LINE.match(line)
    if m is None:
        return None
    g = m.groupdict()
    #@row g: present={ts,level,svc,msg,extra} definite={ts,level,svc,msg,extra} closed
    return Entry(g["ts"], g["level"], g["svc"], g["msg"].strip(), parse_fields(g["extra"]))   #@ ok extra


def parse_log(text):
    entries = []
    for line in text.splitlines():
        e = parse_line(line)
        if e is not None:
            entries.append(e)
    return entries


def mask(text):
    return EMAIL.sub(lambda m: m["user"][0] + "***@" + m["domain"], text)


def aggregate(entries):
    levels = Counter(e.level for e in entries)
    by_svc = defaultdict(list)
    for e in entries:
        by_svc[e.svc].append(e)
    latency = {}
    for svc, es in by_svc.items():
        ms = [int(e.fields["ms"]) for e in es if "ms" in e.fields]   #@ ok ms
        if ms:
            latency[svc] = {"max": max(ms), "avg": sum(ms) / len(ms), "n": len(ms)}
    out = {"levels": levels, "services": dict(by_svc), "latency": latency}
    return out                                     #@row out: present={levels,services,latency} definite={levels,services,latency} closed


def worst(latency):
    ranked = sorted(latency.items(), key=lambda kv: kv[1]["max"], reverse=True)
    return ranked[0][0] if ranked else None
#@schema param worst.latency['*']: always={max} only={max}


def recent_errors(entries, n=2):
    last = deque(maxlen=n)
    for e in entries:
        if e.level == "ERROR":
            last.append({"ts": e.ts, "svc": e.svc, "msg": mask(e.msg), **e.fields})
    return list(last)


def summary_of(entries, agg):
    lv = agg["levels"]
    return {"total": len(entries), "errors": lv["ERROR"], "warnings": lv["WARN"], "slowest": worst(agg["latency"]) or "-"}


def render(summary):
    return REPORT.substitute(summary)
#@schema param render.summary: always={total,errors,warnings,slowest} only={total,errors,warnings,slowest}


def user_fields(entry):
    f = entry.fields
    out = OrderedDict()
    out["svc"] = entry.svc
    if "user" in f:
        out["user"] = f["user"]                    #@ ok user
    out["status"] = f.get("status", "n/a")
    return out                                     #@row out: present={svc,user,status} definite={svc,status} closed


def activity(entries):
    per_user = defaultdict(Counter)
    for e in entries:
        user = e.fields.get("user")
        if user:
            per_user[user][e.level] += 1
    return per_user


LOG_A = """\
10:00:01 INFO [api] request served|ms=120 status=200 user=ann
10:00:02 WARN [db] slow query|ms=900
10:00:03 ERROR [api] failed for bob.smith@example.com|status=500 ms=40
garbage line
10:00:04 INFO [auth] login ok|user=bo
10:00:05 ERROR [db] connection lost
"""
LOG_B = "10:01:00 INFO [cron] tick\n"

if __name__ == "__main__":
    none = parse_line("garbage line")              #@value none: exactly={None}
    entries = parse_log(LOG_A)
    n = len(entries)                               #@value n: exactly={5}
    e0 = entries[0]                                #@row e0: present={ts,level,svc,msg,fields} definite={ts,level,svc,msg,fields} closed
    f0 = e0.fields                                 #@row f0: present={ms,status,user} definite={ms,status,user} closed
    m0 = e0.msg                                    #@value m0: exactly={"request served"}
    f4 = entries[4].fields                         #@row f4: present={} closed
    try:
        f4["ms"]                                   #@ missing ms
    except KeyError:
        pass

    agg = aggregate(entries)
    levels = agg["levels"]                         #@row levels: present={INFO,WARN,ERROR} definite={INFO,WARN,ERROR} closed
    nerr = levels["ERROR"]                         #@value nerr: exactly={2}
    services = agg["services"]                     #@row services: present={api,db,auth} definite={api,db,auth} closed
    lat = agg["latency"]                           #@row lat: present={api,db} definite={api,db} closed
    api = lat["api"]                               #@row api: present={max,avg,n} definite={max,avg,n} closed
    avg = api["avg"]                               #@value avg: exactly={80.0}
    try:
        lat["auth"]                                #@ missing auth
    except KeyError:
        pass
    slow = worst(lat)                              #@value slow: exactly={"db"}
    top_svc = Counter(e.svc for e in entries).most_common(1)[0][0]   #@value top_svc: exactly={"api"}

    errs = recent_errors(entries)
    er0 = errs[0]                                  #@row er0: present={ts,svc,msg,status,ms} definite={ts,svc,msg,status,ms} closed
    masked = er0["msg"]                            #@value masked: exactly={"failed for b***@example.com"}
    er1 = errs[1]                                  #@row er1: present={ts,svc,msg} definite={ts,svc,msg} closed

    summary = summary_of(entries, agg)             #@row summary: present={total,errors,warnings,slowest} definite={total,errors,warnings,slowest} closed
    text = render(summary)                         #@value text: exactly={"5 lines: 2 errors / 1 warnings - slowest=db"}
    try:
        render({"total": 1})                       #@ missing errors
    except KeyError:
        pass
    partial = REPORT.safe_substitute(total=3, errors=0)   #@value partial: exactly={"3 lines: 0 errors / $warnings warnings - slowest=$slowest"}

    u0 = user_fields(entries[0])                   #@row u0: present={svc,user,status} definite={svc,user,status} closed
    st0 = u0["status"]                             #@value st0: exactly={"200"}
    u1 = user_fields(entries[1])                   #@row u1: present={svc,status} definite={svc,status} closed
    st1 = u1["status"]                             #@value st1: exactly={"n/a"}
    acts = activity(entries)                       #@row acts: present={ann,bo} definite={ann,bo} closed
    ann = acts["ann"]                              #@row ann: present={INFO} definite={INFO} closed

    entries_b = parse_log(LOG_B)
    agg_b = aggregate(entries_b)
    lat_b = agg_b["latency"]                       #@row lat_b: present={} closed
    text_b = render(summary_of(entries_b, agg_b))  #@value text_b: exactly={"1 lines: 0 errors / 0 warnings - slowest=-"}
    acts_b = activity(entries_b)                   #@row acts_b: present={} closed

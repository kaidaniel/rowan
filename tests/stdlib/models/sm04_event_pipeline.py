# an event pipeline: a plugin registry filled by decorators, wraps-based enrichers folded with reduce,
# events grouped per session with itertools.groupby, handlers looked up per kind, summaries via callbacks

from collections import Counter, defaultdict
from functools import lru_cache, partial, reduce, wraps
from itertools import chain, groupby
from operator import itemgetter

HANDLERS = {}
ENRICHERS = []


def handles(kind):
    def register(fn):
        HANDLERS[kind] = fn
        return fn
    return register


def enricher(fn):
    @wraps(fn)
    def wrapper(event, **ctx):
        out = fn(event, **ctx)
        out["enriched_by"] = out.get("enriched_by", ()) + (fn.__name__,)
        return out
    ENRICHERS.append(wrapper)
    return wrapper


@enricher
def add_region(event, **ctx):
    out = dict(event)
    out["region"] = ctx["regions"].get(event["country"], "other")   #@ ok regions ; ok country
    return out


@enricher
def add_tier(event, **ctx):
    out = dict(event)
    out["tier"] = "gold" if event.get("amount", 0) >= ctx["gold_at"] else "std"
    return out


@lru_cache(maxsize=32)
def tier_label(tier):
    return {"gold": "Gold member", "std": "Standard"}[tier]


@handles("click")
def on_click(session, ev):
    session["clicks"] = session.get("clicks", 0) + 1
    session["last_target"] = ev["target"]          #@ ok target
    #@row session: present={session,events,clicks,last_target} definite={session,events,clicks,last_target} closed
#@schema param on_click.ev: always={target} only={target}


@handles("purchase")
def on_purchase(session, ev):
    session["revenue"] = session.get("revenue", 0) + ev["amount"]   #@ ok amount
    #@row ev: present={session,kind,sku,amount,country,region,tier,enriched_by} definite={session,kind,sku,amount,country,region,tier,enriched_by} closed
    session.setdefault("items", []).append(ev["sku"])
    session["label"] = tier_label(ev["tier"])
    n = len(session)
    #@row session: present={session,events,clicks,last_target,user,revenue,items,label} definite={session,events,revenue,items,label} closed
#@schema param on_purchase.ev: always={amount,sku,tier} only={amount,sku,tier}


@handles("login")
def on_login(session, ev):
    session["user"] = ev["user"]                   #@ ok user
#@schema param on_login.ev: always={user} only={user}


def enrich(event, ctx):
    return reduce(lambda ev, f: f(ev, **ctx), ENRICHERS, event)


def process(events, ctx):
    events = sorted(events, key=itemgetter("session"))
    sessions = {}
    unknown = Counter()
    for sid, grp in groupby(events, key=itemgetter("session")):
        state = {"session": sid, "events": 0}
        for ev in map(partial(enrich, ctx=ctx), grp):
            state["events"] += 1
            handler = HANDLERS.get(ev["kind"])     #@ ok kind
            if handler is None:
                unknown[ev["kind"]] += 1
                continue
            handler(state, ev)
        sessions[sid] = state
    return sessions, unknown                       #@row unknown: present={scroll} closed
#@schema param process.events['*']: always={session,kind,country} sometimes={target,amount,sku,user} only={session,kind,country}


def summarize(sessions):
    revenue = reduce(lambda acc, s: acc + s.get("revenue", 0), sessions.values(), 0)
    top = max(sessions.values(), key=lambda s: s.get("revenue", 0))
    by_user = {s["user"]: s["session"] for s in sessions.values() if "user" in s}
    items = list(chain.from_iterable(s.get("items", []) for s in sessions.values()))
    out = {"revenue": revenue, "top": top["session"], "users": by_user, "items": len(items)}
    return out                                     #@row out: present={revenue,top,users,items} definite={revenue,top,users,items} closed


def revenue_by_region(events, ctx):
    out = defaultdict(int)
    for ev in map(partial(enrich, ctx=ctx), events):
        if ev["kind"] == "purchase":
            out[ev["region"]] += ev["amount"]
    return dict(out)


def report_line(state):
    return "{session}: {events} events".format(**state)
#@schema param report_line.state: always={session,events} only={session,events}


CTX = {"regions": {"DE": "eu", "FR": "eu", "US": "na"}, "gold_at": 100}

EVENTS_A = [
    {"session": "s2", "kind": "login", "user": "bo", "country": "US"},
    {"session": "s1", "kind": "click", "target": "buy", "country": "DE"},
    {"session": "s1", "kind": "purchase", "sku": "p1", "amount": 120, "country": "DE"},
    {"session": "s2", "kind": "purchase", "sku": "p2", "amount": 30, "country": "US"},
    {"session": "s1", "kind": "scroll", "country": "FR"},
]
EVENTS_B = [
    {"session": "s9", "kind": "click", "target": "home", "country": "JP"},
]

kinds = HANDLERS                                   #@row kinds: present={click,purchase,login} definite={click,purchase,login} closed
ne = len(ENRICHERS)                                #@value ne: exactly={2}

if __name__ == "__main__":
    one = enrich({"country": "FR", "amount": 150}, CTX)
    #@row one: present={country,amount,region,tier,enriched_by} definite={country,amount,region,tier,enriched_by} closed
    reg = one["region"]                            #@value reg: exactly={"eu"}
    tier = one["tier"]                             #@value tier: exactly={"gold"}
    by = one["enriched_by"]                        #@value by: exactly={("add_region", "add_tier")}

    sessions, unknown = process(EVENTS_A, CTX)
    #@row sessions: present={s1,s2} definite={s1,s2} closed
    s1 = sessions["s1"]
    #@row s1: present={session,events,clicks,last_target,revenue,items,label} definite={session,events,clicks,last_target,revenue,items,label} closed
    s2 = sessions["s2"]
    #@row s2: present={session,events,user,revenue,items,label} definite={session,events,user,revenue,items,label} closed
    ev1 = s1["events"]                             #@value ev1: exactly={3}
    lab = s1["label"]                              #@value lab: exactly={"Gold member"}
    lab2 = s2["label"]                             #@value lab2: exactly={"Standard"}
    nsc = unknown["scroll"]                        #@value nsc: exactly={1}
    try:
        s2["clicks"]                               #@ missing clicks
    except KeyError:
        pass

    summary = summarize(sessions)
    rev = summary["revenue"]                       #@value rev: exactly={150}
    top = summary["top"]                           #@value top: exactly={"s1"}
    users = summary["users"]                       #@row users: present={bo} definite={bo} closed
    nitems = summary["items"]                      #@value nitems: exactly={2}

    sessions_b, unknown_b = process(EVENTS_B, CTX)
    #@row sessions_b: present={s9} definite={s9} closed
    s9 = sessions_b["s9"]
    #@row s9: present={session,events,clicks,last_target} definite={session,events,clicks,last_target} closed
    summary_b = summarize(sessions_b)
    rev_b = summary_b["revenue"]                   #@value rev_b: exactly={0}
    users_b = summary_b["users"]                   #@row users_b: present={} closed

    regions = revenue_by_region(EVENTS_A, CTX)     #@row regions: present={eu,na} definite={eu,na} closed
    eu = regions["eu"]                             #@value eu: exactly={120}
    regions_b = revenue_by_region(EVENTS_B, CTX)   #@row regions_b: present={} closed

    line = report_line(s1)                         #@value line: exactly={"s1: 3 events"}
    line9 = report_line(s9)                        #@value line9: exactly={"s9: 1 events"}
    hits = tier_label.cache_info().hits            #@value hits: exactly={0}

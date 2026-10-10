"""Pure-Python aggregation over Event models: collections and itertools."""
from collections import Counter, OrderedDict, defaultdict
from itertools import accumulate, chain, groupby, islice
from operator import attrgetter


def totals_by(events, attr):
    acc = defaultdict(float)
    for e in events:
        acc[getattr(e, attr)] += e.amount
    return dict(sorted(acc.items()))


def counts_by(events, attr):
    return dict(Counter(getattr(e, attr) for e in events))


def top_users(events, n=2):
    c = Counter(e.user for e in events)
    return [u for u, _ in c.most_common(n)]


def runs_by_day(events):
    ordered = sorted(events, key=attrgetter("day", "seq"))
    out = OrderedDict()
    for day, grp in groupby(ordered, key=attrgetter("day")):
        items = list(grp)
        out[day] = {"count": len(items), "users": sorted({e.user for e in items}), "amount": round(sum(e.amount for e in items), 2)}
    return dict(out)


def running_totals(events):
    return list(accumulate(e.amount for e in events))


def source_mix(batches):
    total = sum(len(v) for v in batches.values())
    return {src: {"n": len(v), "share": round(len(v) / total, 3) if total else 0.0} for src, v in batches.items()}


def first_ids(events, n):
    return [e.id for e in islice(events, n)]


def all_tags(events):
    return sorted(set(chain.from_iterable(e.tags for e in events)))


def merge_counts(*dicts):
    out = Counter()
    for d in dicts:
        out.update(d)
    return dict(sorted(out.items()))

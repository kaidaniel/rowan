"""A match on text rowan does not know: a group that may not take part (under `?`, `*`, one side
of `|`) is None (or `groups()`'s / `groupdict()`'s default) when it does not; one every match
has is a str. (Found by the held-out corpus: toml's decoder, `h, m, s, _, ms =
TIME_RE.match(v).groups()`, whose optional milliseconds were typed `str(?)`.)"""
import re

TIME_RE = re.compile(r"([0-9]{2}):([0-9]{2})(\.([0-9]{3,6}))?")
KV_RE = re.compile(r"(?P<key>\w+)=(?P<value>\w+)?|(?P<flag>!\w+)")


def parse_time(v):
    m = TIME_RE.match(v)
    if m:
        h, mi, frac, ms = m.groups()
        #@general h: types={str}
        #@general ms: shows={None} types={str}
        alt = m.groups("0")[3]  #@general alt: shows={'0'} types={str}
        g = m.group(4)  #@general g: shows={None} types={str}
        return ms
    return None


def parse_kv(v):
    m = KV_RE.match(v)
    if m:
        d = m.groupdict()
        key = d["key"]  #@general key: shows={None} types={str}
        return key
    return None


a = parse_time("12:30")  #@value a: includes={None}
b = parse_time("12:30.250")  #@value b: includes={'250'}
c = parse_kv("!x")  #@value c: includes={None}

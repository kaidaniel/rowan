"""Hypothesis: `x = given or DEFAULT` picks the right side when the left is a known-empty
or None value and the left when it is a non-empty record; `d.get("k") or {}` on a missing
key is the empty dict.

Expectations were written from the program text before running the engine."""

DEFAULT = {"host": "h", "port": 1}


def settle(given):
    cfg = given or DEFAULT
    return cfg


a = settle(None)                          #@row a: present={host,port} definite={host,port} closed
b = settle({})                            #@row b: present={host,port} definite={host,port} closed
c = settle({"host": "x"})                 #@row c: present={host} definite={host} closed
d = {"a": {"k": 1}}
sub = d.get("missing") or {}              #@row sub: present={} definite={} closed
have = d.get("a") or {}                   #@row have: present={k} definite={k} closed
c["port"]                                 #@ missing port

"""Hypothesis: chained `.get(k, {}).get(k2, default)` on a known nested dict gives the known
value, the default, or None.

Expectations were written from the program text before running the engine."""

cfg = {"db": {"host": "h"}}
host = cfg.get("db", {}).get("host")
h = {host: 1}                                #@row h: present={h} definite={h} closed
port = cfg.get("db", {}).get("port", 5432)
p = {port: 1}                                #@row p: present={5432} definite={5432} closed
none = cfg.get("cache", {}).get("ttl")
n = {none: 1}                                #@row n: present={None} definite={None} closed

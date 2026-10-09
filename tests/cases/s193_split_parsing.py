"""Hypothesis: parsing a literal `"a=1;b=2"` with split and `dict(...)` of the pairs gives
known keys; `partition` splits a literal into known parts.

Expectations were written from the program text before running the engine."""

raw = "a=1;b=2"
cfg = dict(p.split("=") for p in raw.split(";"))  #@row cfg: present={a,b} definite={a,b} closed
cfg["c"]                                     #@ missing c
host, _, port = "h:80".partition(":")
srv = {host: port}                           #@row srv: present={h} definite={h} closed

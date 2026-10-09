"""Hypothesis: a `ChainMap` over known dicts finds a key in any of them and lacks the keys
none has; `dict(cm)` has the union.

Expectations were written from the program text before running the engine."""

from collections import ChainMap

defaults = {"host": "h", "port": 1}
over = {"port": 2}
cm = ChainMap(over, defaults)
cm["host"]                                   #@ ok host
cm["user"]                                   #@ missing user
merged = dict(cm)                            #@row merged: present={host,port} definite={host,port} closed

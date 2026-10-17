"""Hypothesis: a name read at several places on one line has, at each place, the value it has
there: in `cfg["host"] + ":" + cfg["port"]` the second `cfg` is known to have "host" (the first
read succeeded), the first is not. Its hover shows that place's value, not the places' values as
alternatives (`{**?} | {"host": ?, **?}`, whose note says "depending on the path").
vscode-rowan/test.js checks the hovers.

Expectations were written from the program text before running the engine."""
import json
import sys

cfg = dict(json.loads(sys.argv[1]))
url = cfg["host"] + ":" + cfg["port"]        #@ input host
#@ input port
x = 1
x = x + 1                                    #@value x: exactly={2}
n = len(cfg)                                 #@value n: unknown

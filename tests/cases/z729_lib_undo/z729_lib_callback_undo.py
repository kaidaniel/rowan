"""Followed packages (`--follow`): an entry into a package that runs past the library's share is
undone and is a call that does not run (z716). Here the entry (`cblib.run_all`) has called the
program's callback before it is cut: after the undo, what the callback changes (the dict it is
given, a global) may have changed all the same, and the package's own store (`count`) may have
happened. A later call into the package does not run either. `#@limits budget=64`: the share
is 16,384 of work.

Expectations were written from the program text before running the engine."""
#@argv --follow cblib --site-packages site
#@limits budget=64
import cblib

G = {"n": 0}


def cb(d):
    d["seen"] = 1
    G["n"] = 5


out = {}
r = cblib.run_all([cb], out)
s = out.get("seen", -1)                      #@value s: includes={1}
g = G["n"]                                   #@value g: includes={5}
c = out.get("count", -1)                     #@value c: includes={65536}
#@value r: includes={65536}
out2 = {}
f = cblib.first([cb], out2)                  #@value f: includes={"first"}
s2 = out2.get("seen", -1)                    #@value s2: includes={1}

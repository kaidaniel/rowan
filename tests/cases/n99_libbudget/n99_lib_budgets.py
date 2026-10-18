"""Installed packages followed with per-function budgets (`--follow`): a library function past
its budget is a call that does not run (what its code may change, from its text), the rest of
the package stays analysed; an entry of the program's code into a package (an import, a call)
past its allowance goes on with its later calls not run: an import of a package too expensive
in part still knows its module-level names. `#@limits budget=64`: the import's `expensive()`
(256 calls) is past the import's allowance, so `register("late", 2)` after it does not run.

Expectations were written from the program text before running the engine."""
#@argv --follow partlib --site-packages site
#@limits budget=64
import partlib
from partlib import REGISTRY, cheap, tag_all

name = partlib.NAME                          #@value name: exactly={"partlib"}
t = partlib.TABLE
#@row t: present={0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15} definite={0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15} closed
t[15]                                        #@ ok 15

REGISTRY["early"]                            #@ sound early
#@todo ok early   (`register` only stores: the call that did not run removed nothing)
REGISTRY["late"]                             #@ sound late
#@todo ok late

e = {"k": 1}
s = tag_all(e)                               #@value s: exactly={"done"}
e["last"]                                    #@ sound last
#@todo ok last
e["k"]                                       #@ sound k
#@todo ok k

c = cheap({"k": 1})                          #@row c: present={x} definite={x} closed
cx = c["x"]                                  #@value cx: exactly={1}
REGISTRY["never"]                            #@ fails never
#@todo missing never   (the last line: it raises)

"""Installed packages followed within budgets (`--follow`): library functions have budgets as
the program's do, and each entry of the program's code into a package (a call, an import) an
allowance past which its calls do not run (their arguments may change). `#@limits budget=64`
makes `heavy` (256 calls of `_step`) too much: the import of `heavyimport` (which calls it) uses
up `heavy`'s and `_step`'s budgets, so the program's own calls of `heavy` do not run.

Expectations were written from the program text before running the engine."""
#@argv --follow heavylib,heavyimport --site-packages site
#@limits budget=64
import heavyimport
from heavylib import light, heavy, drop_then_work

a = {"k": 1}
r1 = light(a)                             #@row r1: present={ok} definite={ok} closed
a["seen"]                                 #@ ok seen
a["k"]                                    #@ ok k

b = {"k": 1}
r2 = heavy(b)                             #@value r2: unknown
b["n"]                                    #@ sound n
#@todo ok n
b["k"]                                    #@ sound k
#@todo ok k

v = heavyimport.get()                     #@value v: unknown

c = {"k": 1}
c2 = drop_then_work(c)
c["k"]                                    #@ fails k
#@todo missing k   (the last line: it raises)

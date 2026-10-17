"""Installed packages followed within an allowance (`--follow`): each entry of the program's
code into an installed package (a call, an import) runs as the program's code does if it stays
within its work, else it is undone and is what it is without following: a call rowan does not
run (its arguments may change), an unknown module. `#@limits budget=64` makes `heavy` (256 calls
of `_step`) and the import of `heavyimport` (which calls it) too much for one entry.

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

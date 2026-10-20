"""Deferred definitions and what could tell them from running at the `def` (see
z706_deferred_definitions.py): a decorator reading a name bound only later in its module
raises at the `def` (`partmod`: `early` is None), as does one decorating from a module still
loading (`cycb` in a circular import: `g` is None); decorators that append to a shared list
ran at the `def`, in order (`ORDER`); one reading an attribute a function may not have
(`checks`) raised there; the module's namespace read as a whole (`vars`, `import *`, a name
not known) and unknown code given the module (it may call `bump`, which changes `STATE`) see
the decorated functions. (`e`, `gv`, `ch`: a read that fails goes on in rowan, as after any
failed access, so the name may also hold the function: `includes`, not `exactly`.)

Expectations were written from the program text before running the engine."""
import sys
import timeit

import cyca
import hazlib
import partmod
from cycb import g
from hazlib import *

e = partmod.early                            #@value e: includes={None}
gv = g                                       #@value gv: includes={None}
order = len(hazlib.ORDER)                    #@value order: exactly={2}
o0 = hazlib.ORDER[0]                         #@value o0: exactly={"first"}
ch = hazlib.checked                          #@value ch: includes={None}
vu = vars(hazlib)["unused"](2)               #@value vu: exactly={4}
lab = used.label                             #@value lab: exactly={"first"}
attr = sys.argv[0][:0] + "unused"
dyn = getattr(hazlib, attr)
k1 = hazlib.STATE["k"]                       #@value k1: exactly={1}
timeit.timeit("m.bump(0)", globals={"m": hazlib}, number=1)
k2 = hazlib.STATE["k"]                       #@value k2: includes={1,2}

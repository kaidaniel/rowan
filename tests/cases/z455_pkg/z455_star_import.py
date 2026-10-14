"""Hypothesis: `from M import *` binds the names in M's `__all__`, or, without one, every
name of M not starting with an underscore; a package that star-imports its modules has their
names as attributes, also for a module of the package importing it while it is still being
imported (toolz.curried, timm.models). rowan skipped star imports.

Expectations were written from the program text before running the engine."""

import z455shapes
from z455shapes import *

sq = Square(3)
a = area(sq)                                 #@value a: exactly={9}
p = perimeter(sq)                            #@value p: exactly={12}
pub = PUBLIC                                 #@value pub: exactly={p}
k = z455shapes.registry.KNOWN(sq)            #@value k: exactly={9}
try:
    h = z455shapes.helper                    #@ missing helper
except AttributeError:
    h = None
hv = h                                       #@value hv: exactly={None}
try:
    z455shapes._private                      #@ missing _private
except AttributeError:
    pass

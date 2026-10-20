"""Decorated definitions in a module's code whose decorators are plain (`tag`, `flagit`: they
make a wrapper with `functools.wraps`, store attributes, read only the names they are bound
to) run when the name is first read, not at the `def`; read, they give what running them at
the `def` gives. `tag` reads `LABEL`, which the module rebinds after the `def`s: the wrapper
has the value at the `def` ("first"). Stacked decorators where one is not plain (`register`
fills a registry) run at the `def`, in order: the registry holds the undecorated function.

Expectations were written from the program text before running the engine."""
import decolib
from decolib import REGISTRY, both, flagged, used

v = used(1)                                  #@value v: exactly={2}
lab = used.label                             #@value lab: exactly={"first"}
name = used.__name__                         #@value name: exactly={"used"}
n = len(decolib.CALLS)                       #@value n: exactly={1}
w = decolib.unused(3)                        #@value w: exactly={6}
REGISTRY["both"]                             #@ ok both
raw = getattr(REGISTRY["both"], "label", "none")  #@value raw: includes={"none"}
#@todo value raw: exactly={"none"}   (an attribute the program never gave a function: unknown code may have)
bl = both.label                              #@value bl: exactly={"first"}
fl = flagged.flag                            #@value fl: exactly={True}
inner = flagged.__wrapped__.flag             #@value inner: includes={True}

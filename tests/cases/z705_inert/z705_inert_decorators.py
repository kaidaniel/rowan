"""Installed packages followed (`--follow`): past an import's allowance its calls of code do
not run, but a decorator whose own code is inert (straight-line: it makes a wrapper with
`functools.wraps` and stores an attribute on it) still runs, so the decorated function is the
wrapper, precisely. A registry decorator (`register` stores into `REG`) is not inert: past the
allowance it is a call that does not run, as before. `#@limits budget=64`: the import's
`expensive()` (256 calls) is past its allowance.

Expectations were written from the program text before running the engine."""
#@argv --follow inertlib --site-packages site
#@limits budget=64
from inertlib import REG, double, plain

v = double(3)                                #@value v: exactly={6}
t = double.tagged                            #@value t: exactly={True}
n = double.__name__                          #@value n: exactly={"double"}
r = plain(2)                                 #@value r: includes={3}
REG["plain"]                                 #@ sound plain

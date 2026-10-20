"""Calls of a followed package's code that do not run, each reaching much of the package's code
(`hooklib`: 40 subclasses whose `__init_subclass__` hook reaches 200 helpers), share one closure
of the library's code: it is walked once, extended by what each call adds, and applied again
only where the state changed since it was last applied. What none of that code reaches stays
exact (`CONST`, the program's `CONFIG`); what it may change is unknown after each such call,
also when the program changed it in between (`STATE["n"] = "mine"`, then a subclass whose
hook's call of `h000` does not run: CPython gives "Late"). `#@limits budget=64`: an entry's
allowance is 4 × 64 = 256 of work. `#@visits`: the closure (about 200 functions and the slots
they name) walked once, not once per subclass (41 × that).

Expectations were written from the program text before running the engine."""
#@argv --follow hooklib --site-packages site
#@limits budget=64
#@visits <= 2500
CONFIG = {"mode": "fast"}

import hooklib
from hooklib import REGISTRY

name = hooklib.NAME                          #@value name: exactly={"hooklib"}
c = hooklib.CONST["a"]                       #@value c: exactly={1}
mode = CONFIG["mode"]                        #@value mode: exactly={"fast"}
REGISTRY["C00"]                              #@ sound C00
REGISTRY["C39"]                              #@ sound C39
n = hooklib.STATE["n"]                       #@value n: includes={"C39"}
hooklib.STATE["n"] = "mine"
mine = hooklib.STATE["n"]                    #@value mine: exactly={"mine"}


class Late(hooklib.Base):
    pass


late = hooklib.STATE["n"]                    #@value late: includes={"Late"}
REGISTRY["Late"]                             #@ sound Late
c2 = hooklib.CONST["a"]                      #@value c2: exactly={1}
mode2 = CONFIG["mode"]                       #@value mode2: exactly={"fast"}

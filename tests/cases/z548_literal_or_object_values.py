"""A value that is a literal on some paths and an object on others (`"dev" if c else ["p", "q"]`)
is either: what is done with it is done to each. rowan took the object part alone in calls
of builtins and methods, operators, subscripts and `in`: `len(x)` was 2 (3 on the str's
path), `x * 2` only the list, `x.count("p")` only the list's.

Expectations were written from the program text before running the engine."""
import os

c = os.environ.get("Z548_C") == "1"
x = "dev" if c else ["p", "q"]
n = 3 if c else [1]

a1 = len(x)                              #@value a1: exactly={2, 3}
a2 = x.count("p")                        #@value a2: exactly={0, 1}
a3 = x * 2                               #@value a3: includes={"devdev"}
a4 = "e" in x                            #@value a4: includes={True, False}
a5 = n * 2                               #@value a5: includes={6}
a6 = min(x)                              #@value a6: includes={"d", "p"}
a7 = x.upper() if c else "-"             #@value a7: includes={"DEV", "-"}
#@todo value a7: exactly={"DEV", "-"} (a list has no upper: that path raises AttributeError)
a8 = sorted(x)[0]                        #@value a8: includes={"d", "p"}

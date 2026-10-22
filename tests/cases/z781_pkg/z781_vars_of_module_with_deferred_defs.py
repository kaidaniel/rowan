"""An imported module's decorated definitions (`@wrapped @with_tag("w") def f2`, `@tagged def
f3`) seen through `vars(m)` and as attributes: `update_wrapper` copies the tag set by the inner
decorator onto the wrapper, so `m.f2.tag` is "w" and the wrapper calls `f2`. `vars(m)` holds
every name the module binds, `functools` too (it starts with "f").

The fuzzer's report of an unsound claim here (decorators 1414000110) was its own: two programs
drew one module name, and rowan was checked against the other program's module.
Expectations were written from the program text before running the engine."""
import sys

import tagmod as m

n = len(sys.argv)
ks = sorted(k for k in vars(m) if k.startswith("f"))
first = ks[0]                                #@value first: exactly={"f0"}
last = ks[n % 4]                             #@value last: includes={"f2"}
count = len(ks)                              #@value count: exactly={4}
tag0 = getattr(m.f0, "tag", "-")             #@value tag0: exactly={"-"}
tag2 = getattr(m.f2, "tag", "-")             #@value tag2: exactly={"w"}
tag3 = m.f3.tag                              #@value tag3: exactly={"t"}
name2 = m.f2.__name__                        #@value name2: exactly={"f2"}
r0 = m.f0(2)                                 #@value r0: exactly={-2}
r2 = m.f2(2)                                 #@value r2: exactly={3}
r3 = m.f3(2)                                 #@value r3: exactly={5}

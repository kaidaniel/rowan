"""A module's definitions whose decorators are plain run when first read: of twenty decorated
functions the program reads one, so one decorator runs. By hand: the module's 22 statements
(an import, `def tag`, twenty `def`s), `tag` for `f0` (2), the call (the wrapper's `return`,
`f0`'s), the program's 2; run at every `def`, the decorators alone would be 40 statements.

Expectations were written from the program text before running the engine."""
#@work <= 40
from manydefs import f0

v = f0(1)                                    #@value v: exactly={1}

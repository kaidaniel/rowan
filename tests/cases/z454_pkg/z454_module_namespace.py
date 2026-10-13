"""Hypothesis: imports under `if TYPE_CHECKING:` do not run (the constant is False when the
program runs), so an import cycle that only type checkers see does not make a module read
another before its names are set; and a module whose namespace is changed by
`globals().update(locals())` (pip's vendored pkg_resources) may have names rowan never saw
assigned. Both made pip's names "not defined in the module".

Expectations were written from the program text before running the engine."""

import opts
import reqs
import aside

m = opts.mode                                #@value m: exactly={running}
s = reqs.SUPPORTED[0]                        #@value s: exactly={--index-url}
w = aside.working_set                        #@ sound working_set
k = aside.known                              #@value k: exactly={1}

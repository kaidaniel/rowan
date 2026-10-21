# `if k not in d: raise` then `d[k]`, with k one of several literals and d's keys set on some
# paths only (click's parser.py:400, `self._long_opt[opt]`): the read cannot fail, but the test
# narrows neither key in (it says only that one of them is there).
import sys

d = {}
if len(sys.argv) > 2:
    d["--help"] = 1
if len(sys.argv) > 3:
    d["--verbose"] = 2
opt = "--help" if len(sys.argv) > 1 else "--verbose"
if opt not in d:
    raise SystemExit(0)
o = d[opt]                      #@ sound --help ; sound --verbose
#@todo ok --help ; ok --verbose  (the test's outcome per literal of opt: a relation between opt and d's keys, not kept)
one = "--help"
if one not in d:
    raise SystemExit(0)
p = d[one]                      #@ ok --help

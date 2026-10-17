"""A dict's keys put there under keys of a builtin type are of that type: a word count's keys are
`str(?)` (`for k in counts`, `counts.items()`, `sorted(counts)`) and the dict is
`dict[str(?), int(?)]`. Keys that may come from elsewhere (an input, unknown code, keys of
another type) leave them untyped. They were `?`.

Expectations were written from the program text before running the engine."""
import json
import os

text = os.environ.get("TEXT", "a b a")
words = text.split()
counts = {}
for w in words:
    counts[w] = counts.get(w, 0) + 1
for k in counts:
    kk = k                                  #@value kk: types={str}
for k2, v2 in counts.items():
    kv = k2                                 #@value kv: types={str}
    vv = v2                                 #@value vv: types={int}
top = sorted(counts)
first = top[0] if top else "none"           #@todo value first: types={str}  (keys of a dict joined with a literal lose their type: a join sees no rows)
idx = {w: i for i, w in enumerate(words)}
for k3 in idx:
    ki = k3                                 #@value ki: types={str}
mixed = {0: "zero"}
for w in words:
    mixed[w] = 1
for k4 in mixed:
    km = k4                                 #@value km: includes={0}
#@todo value km: includes={0} types={str}  (the literal keys beside the stored ones: `0 | str(?)`)
loaded = json.loads(os.environ.get("JSON", '{"x": 1}'))
for w in words:
    loaded[w] = 2
for k5 in loaded:
    kl = k5                                 #@value kl: untyped

# What a contract says about an input's parts is bounded in depth (PATH_DEPTH): reading the
# parts of copies (`list(...)`, `dict(...)`, a recursion that copies as it goes, typing_extensions'
# `get_args` loop) no longer adds a level per copy (functional: `args['*'][0][0][0]…`, 17 levels).
# The levels within the bound keep their requirements.
import json
import sys


def thaw(o):
    if isinstance(o, dict):
        return {k: thaw(v) for k, v in o.items()}
    if isinstance(o, (list, tuple)):
        return [thaw(e) for e in o]
    return o


def total(rows):
    n = 0
    for r in rows:
        for x in list(r["items"]):
            n += len(x["parts"])
    return n


data = json.loads(sys.argv[1] if len(sys.argv) > 1 else '[{"items": [{"parts": [1]}]}]')
print(total(data))
#@schema json.loads@L25['*']: always={items}
#@schema json.loads@L25['*']['items']['*']: always={parts}
print(len(thaw(data)))

# any / all / next / `in` on a generator whose items' truth depends on input: each later item is
# taken only on the paths where the earlier ones did not decide, so what it reads is needed
# sometimes, and what it does happens on some paths only.
import json
import os

row = json.loads(os.environ.get("ROW_JSON", '{"a": 0, "b": 1}'))
r1 = any(row[k] > 0 for k in ["a", "b"])        #@ input a ; input b
#@schema json.loads@L7: always={a} sometimes={b}
seen = []


def note(x):
    seen.append(x)
    return x


flags = json.loads(os.environ.get("FLAGS_JSON", "[0, 1]"))
r2 = any(note(v) for v in [0, flags[0], 3, 4])
n2 = len(seen)
#@value n2: exactly={2,3}


def fields(r):
    yield r["x"]                                #@ input x
    yield r["y"]                                #@ input y


rec = json.loads(os.environ.get("REC_JSON", '{"x": 0, "y": 2}'))
r3 = any(fields(rec))
#@schema json.loads@L29: always={x} sometimes={y}
names = json.loads(os.environ.get("NAMES_JSON", '{"p": "z", "q": "a"}'))
r4 = "a" in (names[k] for k in ["p", "q"])      #@ input p ; input q
#@schema json.loads@L32: always={p} sometimes={q}
opts = json.loads(os.environ.get("OPTS_JSON", '{"p": 0, "q": 1}'))
r5 = next((k for k in ["p", "q"] if opts[k]), None)     #@ input p ; input q
#@schema json.loads@L35: always={p} sometimes={q}
lim = json.loads(os.environ.get("LIM_JSON", '{"m": 3, "n": 1}'))
r6 = all(lim[k] > 2 for k in ["m", "n"])        #@ input m ; input n
#@schema json.loads@L38: always={m} sometimes={n}
out = {}


def fill(src):
    out["first"] = src["u"]
    yield src["u"]
    out["second"] = src["v"]
    yield src["v"]


inp = json.loads(os.environ.get("INP_JSON", '{"u": 1, "v": 2}'))
r7 = next(x for x in fill(inp) if x)
_ = 0
#@row out: present={first,second} definite={first} closed

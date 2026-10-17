"""What `open()` gives is a file object: `read()` / `readline()` / its lines are str, bytes in a
binary mode; `write()` gives an int; `with open(…) as f` binds the file itself; `json.load(f)`,
`csv.reader(f)` read it as before (an input; rows of str). The file was an unknown of no type,
and so was everything read from it.

Expectations were written from the program text before running the engine."""
import csv
import json
from pathlib import Path

FN = "/tmp/rowan_z654.txt"
with open(FN, "w") as f:
    n = f.write("a,b\n1,2\n")
    #@value n: types={int}
with open(FN) as f:
    text = f.read()
    #@value text: types={str}
with open(FN) as f:
    for line in f:
        ln = line
        #@value ln: types={str}
with open(FN, "rb") as f:
    data = f.read()
    #@value data: types={bytes}
with open(FN) as f:
    first = f.readline()
    #@value first: types={str}
    rest = f.readlines()
for r in rest:
    r0 = r
    #@value r0: types={str}
with open(FN) as f:
    rows = list(csv.reader(f))
for row in rows:
    for cell in row:
        c0 = cell
        #@value c0: types={str}
with Path(FN).open() as f:
    t2 = f.read()
    #@value t2: types={str}
with open(FN, "w") as f:
    json.dump({"k": 1}, f)
with open(FN) as f:
    cfg = json.load(f)
k = cfg["k"]  #@ input k

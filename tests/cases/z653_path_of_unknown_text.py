"""`Path(x)` with `x` a str not known is a path all the same: its name, suffix and parts are str,
its parent and `p / "a"` paths, `str(p)` a str, `exists()` a bool. It was an unknown of no type
(`?`), and so was everything read from it.

Expectations were written from the program text before running the engine."""
import os
from pathlib import Path

base = os.environ.get("Z653_DIR", "/tmp/z653")
p = Path(base)
name = p.name
#@value name: types={str}
q = p / "out.json"
suf = q.suffix
#@value suf: types={str}
par = q.parent
s = str(par)
#@value s: types={str}
fs = f"{q}"
#@value fs: types={str}
e = q.exists()
#@value e: types={bool}
for part in q.parts:
    pt = part
    #@value pt: types={str}
w = q.with_suffix(".txt")
ws = w.as_posix()
#@value ws: types={str}

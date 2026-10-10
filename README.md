# rowan

> [!WARNING]
> **rowan is experimental.** It is extensively tested (about a thousand example programs,
> checked against real Python, pandas, pydantic, FastAPI and PyTorch runs), but it has not been
> used in practice on real code bases. On large programs (thousands of lines and up) it may time out or crash.

**In VS Code:** [rowan on the Visual Studio Marketplace](https://marketplace.visualstudio.com/items?itemName=Rowan.rowan).

**rowan runs your Python program without running it.** It interprets the program abstractly:
every name holds a *set of possible values* (literals, dicts, lists, instances, functions,
tensors, DataFrames, … or `?` for something it cannot know), every branch is taken, every
function is analysed for every way it is called. From that it tells you:

* **what each variable holds**, in notation that reads like Python;
* **which accesses can fail**: a key or attribute that is definitely, possibly or likely missing;
* **what your program requires of its inputs**: the keys of the JSON, CSV columns, environment
  variables, request bodies and parameters it reads, i.e. the schema the code assumes;
* for PyTorch and numpy: **the shape of every tensor**, and the shape errors.

It knows the standard library, pydantic, FastAPI, pandas, PyTorch and numpy well enough to
compute exact results on known data, and stays sound where it can't: an unknown value is `?`,
never a guess. The engine is written in Rust and analyses a few thousand lines in well under a
second.

## Examples

Each block is a complete program followed by rowan's report for it (trimmed to the relevant
lines). Run `rowan FILE.py` to get the same.

### Keys that may be missing, and typos

```python
import sys

def make_user(row, admin=False):
    user = {"id": row["id"], "name": row["name"].strip().title()}
    if admin:
        user["role"] = "admin"
    return user

u = make_user({"id": 7, "name": "  ada lovelace "}, admin=len(sys.argv) > 1)
before = dict(u)
greeting = f"Hello, {u['name']}!"
role = u["role"]          # only set on some paths
email = u["emial"]        # a typo
```

```
== diagnostics ==
  L14   ⚠     u["role"]     item['role'] possibly missing  (set on some paths only)  [dict@L5]
  L15   ✗     u["emial"]    item['emial'] definitely missing  (all keys known)  [dict@L5]

== inferred schemas (requirements on external inputs) ==
  param make_user.row:
      always   : 'id', 'name'

== module-level values at exit ==
  before = {"id": 7, "name": "Ada Lovelace", "role"?: "admin"}
  greeting = "Hello, Ada Lovelace!"
```

`"role"?` means *on some paths only*. The string methods and the f-string are evaluated
exactly. `make_user` also gets a contract of its own: whoever calls it must pass a `row` with
`id` and `name`.

### The schema your code assumes

```python
import json, os

DEFAULTS = {"retries": 3, "timeout": 10.0}

def load_config():
    cfg = {**DEFAULTS, **json.loads(os.environ["APP_CONFIG"])}
    db = cfg["database"]
    url = f"postgres://{db['user']}@{db['host']}:{db.get('port', 5432)}/{db['name']}"
    if cfg.get("debug"):
        print("tracing", cfg["trace_level"])
    return url, cfg["retries"]
```

```
== inferred schemas (requirements on external inputs) ==
  json.loads@L8:
      always   : 'database'
      sometimes: 'trace_level'
  json.loads@L8['database']:
      always   : 'host', 'name', 'user'
  os.environ:
      always   : 'APP_CONFIG'
```

Nested requirements are tracked to where the data came from. `.get` reads require nothing,
and a read behind a condition is `sometimes`. `retries` comes out as `?`, not `3`, because the
JSON may override it.

### FastAPI and pydantic, through the test client

```python
from typing import Literal
from fastapi import FastAPI, HTTPException
from fastapi.testclient import TestClient
from pydantic import BaseModel, Field

app = FastAPI()
ORDERS = {}

class OrderIn(BaseModel):
    item: str
    qty: int = Field(gt=0)
    size: Literal["S", "M", "L"] = "M"

@app.post("/orders/{oid}")
def create(oid: int, order: OrderIn):
    if oid in ORDERS:
        raise HTTPException(status_code=409, detail="exists")
    ORDERS[oid] = order
    return {"id": oid, "item": order.item, "total": order.qty * 2.5}

client = TestClient(app)
ok = client.post("/orders/1", json={"item": "tea", "qty": "4"})
dup = client.post("/orders/1", json={"item": "tea", "qty": 1})
bad = client.post("/orders/2", json={"item": "tea", "qty": 0, "size": "XL"})
ok_body, ok_code = ok.json(), ok.status_code
dup_code, bad_code = dup.status_code, bad.status_code
```

```
  ok_body = {"id": 1, "item": "tea", "total": 10.0}
  ok_code = 200
  dup_code = 409
  bad_code = 422
  ORDERS = {1: OrderIn(item="tea", qty=4, size="M")}

== inferred schemas (requirements on external inputs) ==
  param create.order:
      always   : 'item', 'qty'
```

Requests go through routing, parameter binding, pydantic validation (`"4"` is coerced to `4`,
`qty=0` breaks `gt=0`, `"XL"` is not a `Literal` option) and the handler. The response is what
the client would read back. The handler itself is also analysed for *any* request: its body
must carry `item` and `qty`.

### pandas: columns through merges and groupbys

```python
import pandas as pd

orders = pd.DataFrame([
    {"order_id": 1, "customer": "ann", "amount": 12.5, "region": "north"},
    {"order_id": 2, "customer": "bo", "amount": 7.0, "region": "south"},
    {"order_id": 3, "customer": "ann", "amount": 20.0, "region": "north"},
])
customers = pd.DataFrame({"customer": ["ann", "bo"], "region": ["north", "south"], "tier": ["gold", "basic"]})

joined = orders.merge(customers, on="customer")
by_region = joined.groupby("region_x", as_index=False)["amount"].sum()
oops = joined["region"]

def monthly(path):
    df = pd.read_csv(path, parse_dates=["ts"])
    df["month"] = df["ts"].dt.month
    return df.groupby(["month", "category"])["price"].mean().reset_index()
```

```
== diagnostics ==
  L13   ✗     joined["region"]   item['region'] definitely missing  (removed/excluded on every path)

== inferred schemas (requirements on external inputs) ==
  read_csv@L17:
      always   : 'category', 'price', 'ts'

== function results (per call context) ==
  monthly [L15] -> frame@L18: {"category": ?, "month": ?, "price": ?}

== module-level values at exit ==
  joined = {"amount": ?, "customer": ?, "order_id": ?, "region_x": ?, "region_y": ?, "tier": ?}
  by_region = {"amount": ?, "region_x": ?}
```

The merge suffixed the clashing `region` column on both sides, so `joined["region"]` can't
work. `monthly` is never called, so rowan analyses it with an unknown file: you get the
columns it needs and the columns it returns.

### PyTorch: shapes, symbolically

```python
import torch
import torch.nn as nn
import torch.nn.functional as F

class Attention(nn.Module):
    def __init__(self, n_embd, n_head):
        super().__init__()
        self.n_head = n_head
        self.qkv = nn.Linear(n_embd, 3 * n_embd)
        self.proj = nn.Linear(n_embd, n_embd)

    def forward(self, x):
        B, T, C = x.shape
        q, k, v = self.qkv(x).split(C, dim=2)
        q = q.view(B, T, self.n_head, C // self.n_head).transpose(1, 2)
        k = k.view(B, T, self.n_head, C // self.n_head).transpose(1, 2)
        att = F.softmax(q @ k.transpose(-2, -1), dim=-1)
        y = (att @ v.view(B, T, self.n_head, C // self.n_head).transpose(1, 2)).transpose(1, 2)
        return self.proj(y.reshape(B, T, C))

attn = Attention(64, 4)
out = attn(torch.randn(2, 10, 64))
bad = attn(torch.randn(2, 10, 32))
```

```
== tensor shapes ==
  L15   ✗     shape mismatch: Linear(64 -> 192) given last dim 32  via L25
  L14   needs x.dim() == 3
  L15   needs x.shape[2] == n_embd

== module-level values at exit ==
  out = Tensor[2, 10, 64]
```

Inside `forward` (what the editor hover shows) every tensor has a shape in the function's own
terms: `q` is `Tensor[B, self.n_head, T, n_embd // self.n_head]`, `att` is
`Tensor[B, self.n_head, T, T]`. An undecided condition becomes a *requirement* (`x` must be 3-D,
its last dim `n_embd`); a decided one that fails is a shape error, at the call that triggers it.

### Exact values from the standard library

```python
import datetime, json, re, sqlite3

LOG = """2024-03-01T10:00 INFO user=ann amount=12.5
2024-03-01T11:30 WARN user=bo amount=7
2024-03-02T09:15 INFO user=ann amount=20"""

PAT = re.compile(r"(\S+) (\w+) user=(\w+) amount=([\d.]+)")
rows = [PAT.match(line).groups() for line in LOG.splitlines()]

db = sqlite3.connect(":memory:")
db.execute("CREATE TABLE ev (ts TEXT, user TEXT, amount REAL)")
db.executemany("INSERT INTO ev VALUES (?, ?, ?)", [(ts, user, float(amt)) for ts, _, user, amt in rows])
per_user = dict(db.execute("SELECT user, SUM(amount) FROM ev GROUP BY user ORDER BY user").fetchall())

first = datetime.datetime.fromisoformat(rows[0][0])
label = first.strftime("%A %d %B, %H:%M")
summary = json.dumps({"users": per_user, "since": label}, sort_keys=True)
```

```
  rows = [("2024-03-01T10:00", "INFO", "ann", "12.5"), ("2024-03-01T11:30", "WARN", "bo", "7"), ("2024-03-02T09:15", "INFO", "ann", "20")]
  per_user = {"ann": 32.5, "bo": 7.0}
  label = "Friday 01 March, 10:00"
  summary = "{\"since\": \"Friday 01 March, 10:00\", \"users\": {\"ann\": 32.5, \"bo\": 7.0}}"
```

A regex engine, an in-memory SQL simulation, `datetime` and `json` all produce the values
CPython would. With a log read from outside, the same lines come out as `?`: rowan never
guesses a value it can't compute.

## Reading the report

| mark | meaning |
| --- | --- |
| `✓` | the access always works (`✓~`: unless unknown code removed the key) |
| `⚠` | possibly missing: set on some paths only, or on some of the objects the name may hold |
| `✗` | definitely missing: all keys known, or the key was deleted / renamed / suffixed away |
| `⚠~` | likely missing: only a call rowan can't see into could have added it |
| `◌` | depends on the input: becomes part of the inferred schema |

In values, `?` is something unknown; `a | b` is one of several values; `"k"?:` is a key present
on some paths only; `**?` means other keys may exist; `list[X]` is any number of elements like
`X`; `Tensor[B, T, 4*C]` is a tensor shape. Inside a function, unknowns are named by where they
come from (`row["id"]`, `x.shape[0]`, `json.loads(text)`).

## Building and running

You need a Rust toolchain (stable, from [rustup](https://rustup.rs)). Python is not needed to
run rowan; it never executes your program.

```
cd rowan
cargo build --release                       # → rowan/target/release/rowan
cargo install --path .                      # optional: puts `rowan` on your PATH (~/.cargo/bin)
```

```
rowan prog.py                 # text report: diagnostics, schemas, function results, values
rowan prog.py --json          # JSON lines (verdicts, schemas, summaries)
rowan prog.py --facts         # per-name facts, as the editor integration uses them
rowan serve                   # a long-running server speaking JSON lines over stdio
```

Imports of local modules and packages are followed. Installed packages are modelled
(pandas, pydantic, …) or unknown, unless you ask for their source with `--follow PKG`.
Other options: `--no-entry` (don't analyse uncalled functions with unknown arguments),
`--search-path DIR`, `--site-packages DIR`, `--value-depth N`, `--no-rows`, `--no-summaries`.

**VS Code.** Install [rowan from the Visual Studio Marketplace](https://marketplace.visualstudio.com/items?itemName=Rowan.rowan)
(the engine is bundled for macOS and Linux): hover over a name to see its values, keys and
requirements, with failing accesses as diagnostics. Its source is in `vscode-rowan/`; the
`.vsix` packages of each version are attached to the
[GitHub releases](https://github.com/kaidaniel/rowan/releases).

## More

* `SPECIFICATION.md` covers how rowan works: the value domain, the models, how calls,
  loops, exceptions and generators are interpreted, what is sound and what isn't.
* `AGENTS.md` holds notes for working on the engine: tests, conventions, known problems.
* `tests/` has about a thousand example programs, each annotated with what rowan must infer
  and checked against real Python, pandas, pydantic, FastAPI and PyTorch runs.

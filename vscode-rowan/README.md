> **⚠ Experimental.** `rowan` is in development. Expect it to crash on large files and outputs to be at times poorly explained.

rowan is an **abstract interpreter for Python**: it runs your program on *sets of possible
values* instead of real ones. **Hover over any name** to see what it can hold, what the code
needs of it, and what each of your program's calls gives it. Accesses that may fail are
underlined.

## Dicts: what a function needs, and which call breaks it

```python
def make_user(row, admin=False):
    user = {"id": row["id"], "name": row["name"].strip().title()}
    if admin:
        user["role"] = "admin"
        user["since"] = row["since"]
    return user


ada = make_user({"id": 1, "name": "  ada lovelace ", "since": 1843}, admin=True)
bob = make_user({"id": 2, "name": "bob"}, admin=True)
eve = make_user({"id": 3, "name": "eve"})
```

Hovering `row` shows:

> ```python
> row = (parameter)
> ```
>
> **Requirements**
> - `row["id"]`
> - `row["name"]`
> - `row["since"]` (only if `admin`)
>
> **In Calls**
> - `{"id": 1, "name": "  ada lovelace ", "since": 1843}` on line 9.
> - `{"id": 2, "name": "bob"}` on line 10: ✗ `row["since"]`
> - `{"id": 3, "name": "eve"}` on line 11.
>
> **Notes**
> - `(only if x)`: needed only on calls where `x` holds of the arguments as passed
> - ✗: fails on every path

and hovering `ada` shows the value computed exactly, string methods included:

> ```python
> ada = {
>   "id": 1,
>   "name": "Ada Lovelace",
>   "role": "admin",
>   "since": 1843
> }
> ```
>
> **Allocation Sites**
> - created on line 2

## Tensors: shapes in the function's own terms

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
        v = v.view(B, T, self.n_head, C // self.n_head).transpose(1, 2)
        att = F.softmax(q @ k.transpose(-2, -1), dim=-1)
        y = (att @ v).transpose(1, 2).reshape(B, T, C)
        return self.proj(y)


attn = Attention(64, 4)
out = attn(torch.randn(2, 10, 64))
bad = attn(torch.randn(2, 10, 32))
```

Hovering `att`:

> ```python
> att = Tensor[B, self.n_head, T, T]
> ```
>
> **In Calls**
> - `Tensor[2, 4, 10, 10]` on line 25.
>
> **Notes**
> - `B`: `x.shape[0]`
> - `T`: `x.shape[1]`

Hovering `x`:

> ```python
> x = (parameter)
> ```
>
> **Requirements**
> - `x.dim() == 3`
> - `x.shape[2] == n_embd`
>
> **In Calls**
> - `Tensor[2, 10, 64]` on line 25.
> - `Tensor[2, 10, 32]` on line 26: ✗ (Linear(64 -> 192) given last dim 32)
>
> **Notes**
> - ✗: fails on every path

## What a hover shows

`name = value`, written as Python (a parameter: `name = (parameter)`, with its type or default
when known), then:

* **Requirements**: what the code reads of a parameter for any call (`row["id"]`, `row["since"]
  (only if admin)`, `x.dim() == 3`), or of an input read on that line (`cfg = json.loads(text)`);
* **In Calls**: what each of your program's own calls gives or passes, with the reads that fail
  for it;
* **Allocation Sites**: where its objects were made, and keys they never have;
* **Notes**: what each mark and notation used above means; last, when the analysis was cut
  short or ran function by function.

In values, `?` is something rowan cannot know, `a | b` one of several values, `"k"?:` a key
present on some paths only, `**?` other keys that may exist, and `Tensor[B, T, 4*C]` a tensor's
shape.

Besides Python's builtins and standard library, rowan models PyTorch, numpy, pandas, pydantic
and FastAPI. Your own modules are followed through their imports; other installed packages are
taken to do anything to what they are given.

Files are analysed when opened or saved, or with **Rowan: Analyse current file**. When a whole
file takes longer than a second, rowan answers each function when you first hover in it, for
any call of it (usually well under a second), and says so in the status bar.

Source, documentation and issues: [github.com/kaidaniel/rowan](https://github.com/kaidaniel/rowan).

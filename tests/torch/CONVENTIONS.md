# Writing torch cases

Cases in `small/` (one API area each, 3–10 checks) and `models/` (well-known networks,
100–300 lines, 15+ checks). Expectations are written **by hand from the program text and
PyTorch's documented semantics, before rowan is run**; rowan is never used to produce or
adjust them. Each case must agree with the runtime oracle:

```
uv run --no-project --with torch --with numpy python tests/torch_oracle.py tests/torch/small/s001_matmul_params.py
```

New cases go into `PENDING` (one path per line, relative to `tests/torch/`) until the engine
passes them.

## Program shape

* Plain functions and `nn.Module` classes, and a driver under `if __name__ == "__main__":`
  that calls **every** function or method that has directives, with concrete tensors
  (the oracle fails on a directive line that never ran). The driver's own lines may carry
  directives too (concrete shapes: `y = model(x)  #@shape y: [32, 10]`).
* A deliberate mismatch sits in its own function. The driver calls it inside
  `try: … except (RuntimeError, ValueError, IndexError, TypeError, AssertionError): pass`, and the
  failing line carries `#@shape-error`.
* `torch.manual_seed` is set by the oracle; cases must not depend on random values (no
  data-dependent shapes except where the case is about them: masks, `nonzero`).

## Directives

Put a directive at the end of the statement's (first) line, or on a comment line of its
own directly below (it applies to the code line above).

* `#@shape NAME: [d1, d2, …]`: NAME's shape after the line ran. A tensor, or a
  `torch.Size` / shape tuple.
  * At most one rest `*R`. `*` means unknown dims, and `*_` accepts any rest.
  * `?` means rowan must say the dim is unknown. `_` accepts anything; don't overuse it.
  * `#@shape NAME: ?` means nothing is known, and `none` means not a tensor.
  * `[…] if C1, C2` is a conditional shape (see matmul below).
  * `where A=…, B=…` defines abbreviations, which are substituted before comparing.
* `#@dim NAME: EXPR`: an int value (a dim, a rank, a numel).
* `#@requires C1; C2`: exactly the requirements rowan records on that line (`none` means
  none). Leave the directive off a line to not check it.
* `#@shape-error [text]`: a definite mismatch on that line. Rowan must not report a shape
  error on any other line.
* `#@shape@L40 NAME: …`: only for the runs called from line 40 of the case, i.e. a
  function body analysed in the driver's concrete call.

Several checks on one line: `#@shape a: […]; b: […]`, `#@dim B: …; T: …`.

## What the expectations say

Without `@L`, a check inside a function is about the function analysed **on its own,
with open parameters**: every parameter is an unknown value of unknown shape. In a
method, `self` is the instance `__init__` builds from open arguments. So write what holds
for *any* call, in terms of the function's own names:

* **The dims of a tensor-valued name `x`** (a parameter, `self.w`, `batch["ids"]`, the
  result of an unknown call) are `x.shape[i]`. Indices are non-negative once the rank is
  known, e.g. after `B, T, C = x.shape`, `x.dim() == 3` was required, or the value was
  built with known rank. They stay negative while the rank is unknown: `x.shape[-1]`.
* **A rest** is a slice of one name's shape: `*x.shape[:-1]`, `*x.shape[1:]`,
  `*x.shape[2:-1]`. A parameter nobody looked into is `[*x.shape]`.
* **Named ints** are the program's own terms: `n`, `self.n_embd`, `cfg.d_model`,
  `config["hidden"]`.
  * In a method, a value that came from an `__init__` argument is written as the
    shortest attribute path from `self` holding it (`self.n_embd`, then
    `self.config.n_embd`).
  * If no self attribute holds it, write the `__init__` expression in `__init__`'s
    names: `d_model`, `config.n_embd`, `d_model // n_heads`.
* **Arithmetic** is ordinary: `4*C`, `T*C`, `x.shape[1] - 1`, `(H + 2*1 - 3)//2 + 1`. The
  harness compares as polynomials; `//` and `%` terms compare textually after
  normalising their operands. Fold constants when you can: `(32 + 2 - 3)//1 + 1` is
  `32`.
* **Broadcasting:**
  * equal dims stay; a literal 1 gives the other dim;
  * otherwise `max(a, b)`;
  * two rests broadcast to `broadcast(r1, r2)`, and a known part as a list:
    `broadcast([x.shape[0]], y.shape[:-2])`;
  * an unknown operand that may be a tensor (an unknown parameter `s` in `x * s`) gives
    `[*broadcast(x.shape, s.shape)]`;
  * literals, arithmetic on ints and dims, and `math.*` results are scalars.
* **Slices** assume dims ≥ 1:
  * `x[:, :T]` gives `min(T, x.shape[1])` (folded when both are literal);
  * `x[:, 1:]` gives `x.shape[1] - 1`;
  * `x[:, a:b]` gives `min(b, d) - a` only when both bounds are literal; otherwise `?`;
  * a boolean mask or `nonzero` gives `?`.
* **`x @ y` / `matmul`** where a rank is unknown is the ≥2-D case under assumptions:

  ```
  [*broadcast(x.shape[:-2], y.shape[:-2]), x.shape[-2], y.shape[-1]] if x.dim() >= 2, y.dim() >= 2
  ```

  Only the operands whose rank is unknown appear in the `if`. Known ranks pick the exact
  case: 1-D @ 1-D is `[]`, `(n, m) @ (m,)` is `[n]`.
* **Joins:** after `if`/`else` with different shapes, dims that differ become `?`. Two
  ranks that differ keep the common trailing dims, with the leading part an unknown rest
  `*`.
* **Loops** whose body preserves the shape keep it; otherwise the changing dims become
  `?`.

## Requirements (`#@requires`)

A requirement is a condition an operation needs that rowan cannot decide at that point.
If it decides the condition is false, that is a `#@shape-error` instead, and if it
decides it true, nothing is recorded. Forms:

* `a == b`. Write it however you like; the harness compares `a - b` up to sign.
* `x.dim() == 3` (an exact rank: unpacking `B, T, C = x.shape`, `bmm`, `Conv2d` 4-D only,
  …).
* `x.dim() in (2, 3)` (RNN-family, `Conv1d`/`Conv2d` batched or not). Write the tuple
  sorted.
* `x.dim() >= 3` (an op touching dim 2 or -3 of a tensor of unknown rank: `transpose(1, 2)`,
  `x[:, :, 0]`, `sum(dim=2)`). Bounds below 2 are not recorded.
* A requirement that only applies under an assumption: `x.shape[-1] == y.shape[-2] if
  y.dim() >= 2`.

Once a requirement holds (the program went on), it becomes a **fact** on that path:

* a fact is never recorded again on the same path;
* an exact rank turns negative indices into non-negative ones;
* an equality rewrites one side into the other: a literal wins, then a named int
  (`d_in`, `self.n_embd`, `n`) over a shape dim, then the dim of the earlier parameter;
* conditional requirements give no facts;
* `assert x.shape[-1] == 512`, `assert x.dim() == 3` and `if x.dim() != 3: raise …` give
  facts too, but are not requirements.

Element counts in `view`/`reshape` are never requirements. They are checked only when
every dim is literal, and a mismatch then is a `#@shape-error`.

## Rank and dim rules worth remembering

* `Linear(i, o)`: `[*, i] → [*, o]`. `Embedding(n, d)`: `[*] → [*, d]`.
  `LayerNorm(d)`: same shape, last dim d.
* `RNN`/`GRU`/`LSTM(i, h, num_layers=L, batch_first=bf, bidirectional=bi)`:
  * input `(T, N, i)`, or `(N, T, i)` if `bf`, or unbatched `(T, i)`;
  * output `(…, D*h)` with `D = 2 if bi else 1`;
  * `h_n` `(D*L, N, h)`, always batch-second, or `(D*L, h)` unbatched;
  * LSTM returns `out, (h_n, c_n)`.
* `Conv2d(ci, co, k, stride=s, padding=p, dilation=d)`: `(N, ci, H, W)` or `(ci, H, W)`, with
  `H' = (H + 2p - d(k-1) - 1)//s + 1`; `padding="same"` keeps H (stride 1).
  `ConvTranspose2d`: `H' = (H-1)s - 2p + d(k-1) + output_padding + 1`.
  `MaxPool2d(k, s=k, p=0)` and `AvgPool2d` use the conv formula (`AvgPool2d`: d=1).
* `MultiheadAttention(E, h, batch_first=bf)`:
  * `attn(q, k, v)` returns `(out, weights)`;
  * `out` has q's shape (with last dim E);
  * the weights are `(N, L, S)` (averaged over heads), or `None` with
    `need_weights=False`.
* `torch.cat` sums the dim and needs the others equal. `stack` inserts a new dim of size
  len(list). `chunk(n)`/`split(s)` return tuples.

## Clarifications (from writing the first 132 cases)

* **Aliases** are attributes the program assigns on `self` itself (`self.d = d`,
  `self.config = config` → `self.config.n_embd`), never a layer's config attributes
  (`self.fc.in_features`). An attribute holding a compound value (`self.head_dim = d // h`)
  prints as that attribute.
* **Floor division** is compared in a normal form (both in expectations and engine output):
  whole multiples of a monomial divisor leave the floor (`(H - 2)//2 + 1` == `H//2`,
  `(S - p)//p + 1` == `S//p`), nested floors with literal divisors merge (`H//2//2` ==
  `H//4`), constants fold. Write either form.
* **Rank values** are written `x.dim()` (also for `x.ndim`, `len(x.shape)` on unknown rank).
* **Broadcasting** two dims that are not known equal records no requirement (result
  `max(a, b)`); `>= 2` rank bounds are recorded (`Softmax(dim=1)` on unknown rank).
* **Requirements of a callee** are recorded at the callee's lines and, in the caller's
  analysis, attached to the caller's call line through the call chain; they become facts in
  the caller on that path. A `#@requires` line lists what is recorded there in that line's
  own function's analysis (entry run) plus, for a call line, the callees' requirements in
  the caller's names.
* **Defaults**: a parameter with a default is "the default or anything" in the function's own
  analysis; give shape hyper-parameters no default when a check depends on them.
* **Oracle limits**: no lambda on a directive line (the lambda's frame would evaluate it); a
  line that raises cannot carry a `#@shape` check; a closure's dims must be free variables of
  the closure for the oracle to evaluate them; never rebind a name that a check expression
  uses (`h = conv(x)`, not `x = conv(x)`, when checks mention `x.shape`).

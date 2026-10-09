# PyTorch tensor shapes — plan

Working plan for the torch feature. It is the checklist to follow, so tick items here as
they land (one commit per step, only when the golden suite, snapshots and `cargo test`
pass). Agreed with the owner on 2026-10-09:

* Dims are shown as short letters with a legend (`Tensor[B, T, 512]`, `B, T, C = x.shape`).
* A definite mismatch is a diagnostic. A possible mismatch is a *requirement*, shown in the
  hover and in the report, not a warning.
* dtype and device are out of scope.
* Inference inside functions matters most (uncalled `forward`, helpers with tensor
  parameters). Instantiating a model and calling it must work too and is tested.
* Tests: at least 100 small programs over most of the covered API and at least 15 larger
  modules implementing well-known networks (one is a transformer written from scratch).
  They live in `tests/torch/{small,models}/`. Expectations are written by hand from the
  program text before the engine is run. Iterate until there are no known bugs and every
  case passes.

## 1. Domain

* **Dim expression** (`DimE`): a normalised polynomial with integer coefficients over
  *atoms*:
  * `x.shape[i]` / `x.shape[-j]`, a dim of the shape symbol of a named unknown `x`;
  * a named unknown int (`n`, `self.n_embd`, `cfg.d_model`), using the existing symbols;
  * `a // k`, `ceil`, and `max(a, b)` (broadcast of two dims that are not known equal, or
    1 against something);
  * `min(a, b)` (a slice);
  * `?`, an unknown dim, never equal to anything, not even another `?`.

  Equality of two dims is decided as follows. Equal normal forms are equal. A difference
  that normalises to a nonzero constant means definitely different, and so do two
  different literals. Anything else is unknown.
* **Shape**: `prefix dims, [*rest], suffix dims`. `rest` is a list term: `x.shape[a:-b]`,
  `broadcast(r1, r2)`, or `*` (unknown rank). Known rank means no rest. Negative atoms are
  rewritten to positive ones once the rank is known.
* **Values**: a tensor is `KeyVal::Data` with a reserved label (`\x01T<id>`) indexing an
  interned shape; a symbolic int is `\x01D<id>`, an interned `DimE`. Everything that
  knows nothing about torch keeps treating them as unknown, which is sound. `kv_join` on
  two tensors joins their shapes dim by dim: unequal dims become `?`, and unequal ranks
  keep the common suffix with an unknown rest. `x.shape` of known rank is a tuple object
  whose slots are dims, so unpacking, indexing, `len` and `*` work through the existing
  machinery. With unknown rank it is a size label with hooks for unpacking, indexing and
  slicing.
* **Unknowns used as tensors**: when an unknown value (a parameter, a placeholder, an
  attribute read, an unknown call) meets a tensor operation, its shape is the shape
  symbol of its name (`x.shape`). If it has no name, the shape is `[*]`. Tensor
  operations here are: torch functions, `nn` layers, `@`, tensor methods in a module that
  imports torch, and a `Tensor` annotation.
* **Relational facts in `State`** (`tfacts`, joined by intersection):
  * the rank of a shape symbol (`== r` or `>= r`);
  * dim equalities, a substitution oriented towards literals and then older atoms.

  Facts come from:
  * unpacking (`B, T, C = x.shape`);
  * `assert x.shape[-1] == d` and `if x.dim() != 3: raise`;
  * every operation that requires something: the program went on, so the requirement
    held. This is the counterpart of "a successful access narrows the key in".

  Shapes are normalised through the facts when they are read and when they are printed.
* **Requirements**: (function, line, constraint text, e.g. `x.shape[-1] == 512` or
  `x.dim() in (2, 3)`). A requirement is recorded when an operation's condition is
  undecided. If it is decided false, the result is a diagnostic `✗shape` with a message:
  `Linear(512 → 10) needs the last dim to be 512, got 256`.

## 2. API covered

* **Creation:**
  * `tensor` (nested literal lists), `zeros`, `ones`, `empty`, `rand`, `randn`, `full`
    (sizes as varargs or a tuple);
  * `arange`, `linspace`, `eye`, `*_like`, `randint`;
  * `tril`, `triu`, `nn.Parameter`, `register_buffer`.
* **Elementwise ops:** with broadcasting, through `+ - * / ** // %`, comparisons,
  `torch.where`, `masked_fill`, the activations, `F.*` elementwise ops, `clamp`, `exp`,
  `sqrt`, …
* **Matmul:** `@`, `matmul`, `mm`, `bmm`, `einsum` (the equation string is parsed),
  `F.linear`.
* **Shape ops:**
  * `view`, `reshape` (with `-1`, divided symbolically when exact), `view_as`;
  * `permute`, `transpose`, `t`, `.T`, `mT`;
  * `unsqueeze`, `squeeze`, `flatten`, `unflatten`, `expand`, `expand_as`, `repeat`;
  * `contiguous`, `to`, `float`, `detach`, … (identity);
  * `chunk`, `split` (tuples), `unbind`, `cat`, `stack`.
* **Reductions:** `sum`, `mean`, `max`, `min`, `amax`, `argmax`, `std`, `var`, `norm`,
  `logsumexp`, `softmax`, `log_softmax` (with `dim`/`keepdim`). `max(dim)` and `topk`
  return tuples.
* **Indexing:** ints, slices with known or symbolic bounds (`min`), `None`, `...`, and
  tensor or bool masks (`?` dim).
* **Queries:** `.shape`, `.size()`, `.size(i)`, `.dim()`, `.ndim`, `.numel()`,
  `len(x)`.
* **`nn` layers:**
  * `Linear`, `Bilinear`, `Embedding`;
  * `LayerNorm`, `RMSNorm`, `BatchNorm1d/2d`, `GroupNorm`, `Dropout`;
  * the activations, `Softmax(dim)`, `Identity`, `Flatten`, `Unflatten`;
  * `Conv1d/2d`, `ConvTranspose2d`, `MaxPool2d`, `AvgPool2d`, `AdaptiveAvgPool2d`,
    `Upsample` (the size formulas);
  * `RNN`/`LSTM`/`GRU` (2 or 3 dims, `batch_first`, `num_layers`, `bidirectional`, output
    tuples with `h_n`/`c_n`);
  * `MultiheadAttention` (`batch_first`, returns a tuple);
  * `TransformerEncoderLayer`/`TransformerEncoder`;
  * `Sequential`, `ModuleList`, `ModuleDict` (indexing, iteration, chaining).
* **Functional:** the `F.*` versions of the above, plus `cross_entropy`, `mse_loss`, … →
  `[]` (or the unreduced shape when `reduction="none"`), `one_hot`, `pad`, `interpolate`,
  `scaled_dot_product_attention`, `embedding`.
* **User modules:** `class M(nn.Module)`: `super().__init__()` is a no-op, `m(x)` calls
  `m.forward(x)`, and attributes that hold layers hold layer objects.
* **Layer objects:** `Kind::Layer` with the type name in the payload, and the config
  (`in_features`, `weight`, …) as attribute slots, so they can be read and printed.

## 3. Inside functions (the main use)

* Parameters of uncalled functions are named unknowns. Their shapes are shape symbols,
  refined by the facts. An uncalled method's `self` is the instance that `__init__` builds
  with open arguments (the existing entry instance). Layer dims are therefore `__init__`
  symbols. In the methods of that class they print through a self attribute that holds
  the same value (`self.n_embd`). If none does, they print as the `__init__` argument
  expression (`config.n_embd`) with the legend `config: __init__ argument`.
* Across calls: a tensor operation on a placeholder is a summary event. A call site whose
  argument has a known or named shape tabulates (it is analysed concretely). If that
  turns out too slow on the model cases, the next step is substituting shape symbols at
  instantiation.

## 4. Output

* **Hover** (value line plus legend), e.g.:

  ```
  h: Tensor[B, T, 4*C]
     B, T, C = x.shape
     requires x.shape[-1] == self.n_embd  (L23)
  ```

  * Dim names: a local the program bound to exactly that dim (`B`). Otherwise a self
    attribute or a named int (`self.n_head`). Otherwise a generated capital letter (`A`,
    …, skipping letters in use), with the legend saying what it is (`A = x.shape[0]`,
    `*R = x.shape[:-1]`).
  * The parameter's own hover shows its refined shape and the requirements on it.
* **Report:**
  * a section `== tensor shapes ==` (per function: the requirements on parameters);
  * `✗shape` diagnostics in the diagnostics section;
  * `--facts` gains `"shape"` (structured) and `"requires"`;
  * golden JSON gains `shapes` and `shape_errors`.
* **Harness directives:**
  * `#@shape NAME: [dims] [where A=…, B=…]`. Dims are compared as polynomials after
    substituting the `where` names. `*` is the unknown rest, `*x.shape[:-1]` a named
    rest, `?` an unknown dim. `#@shape NAME: ?` means nothing is known. The harness
    checks the joined value of NAME at that line, like `#@value`.
  * `#@requires CONSTRAINT` means a requirement equal to this one is recorded on that
    line.
  * `#@shape-error` means a definite mismatch on that line. A shape error on a line
    without `#@shape-error` fails the case (soundness).

## 5. Steps (each one commit)

0. [x] This plan. Harness support for `tests/torch/` (cases there are always cases) with
   `tests/torch/PENDING` listing cases not yet expected to pass, plus the new directives.
1. [x] Write the cases first: at least 100 small programs in `tests/torch/small/` (one
   API area each, several checks, some with deliberate mismatches) and at least 15
   models in `tests/torch/models/`:
   * a transformer from scratch (MHA, FFN, blocks, positional encoding, causal mask);
   * nanoGPT;
   * ResNet BasicBlock/Bottleneck + downsample;
   * LeNet-5;
   * VGG;
   * AlexNet;
   * U-Net;
   * ViT;
   * DCGAN G/D;
   * VAE;
   * LSTM tagger;
   * seq2seq GRU with attention;
   * MLP-Mixer;
   * word2vec skip-gram;
   * PointNet;
   * BERT embeddings + pooler;
   * a TCN (dilated Conv1d).

   Each has hand-written `#@shape` checks inside `forward` and helpers (uncalled), and at
   least one instantiated `model(x)` with concrete shapes. All start in `PENDING`.
2. [x] Domain: `DimE` and `Shape` interners, normalisation, equality, join, printing;
   tensor and dim labels; `tfacts` in `State`. Unit tests in `cargo test`.
3. [x] Creation, queries (`shape`/`size`/`dim`/unpacking), arithmetic on dims,
   elementwise ops and broadcasting, matmul, shape ops, reductions, indexing.
   Unknowns used as tensors.
4. [x] Requirements and diagnostics; facts from asserts, `if … raise` and operations;
   report and facts output.
5. [x] `nn.Module` subclasses, layer objects, Linear / Embedding / norms / Dropout /
   activations / Sequential / ModuleList / ModuleDict, `F.*`; entry `self` through
   `__init__`; dim naming through self attributes.
6. [x] Conv / pool / upsample formulas, RNN family, MultiheadAttention, Transformer
   layers, `einsum`, `chunk`/`split`/`cat`/`stack`, `scaled_dot_product_attention`.
7. [x] Cross-function precision (tabulation on tensor events; measure; substitution if
   needed).
8. [x] Hover rendering (letters + legend + requirements) in `extension.js` and
   `test.js`; rebuild the vsix.
9. [x] Iterate: run everything, fix every surfaced bug, move cases out of `PENDING`,
   until `PENDING` is empty. Then write more hypothesis cases for weak spots.
10. [x] Docs: README (notation for tensors), SPECIFICATION (§ tensor shapes), AGENTS.md
    (≤ 200 lines); delete this plan, or fold what remains into AGENTS.md.

## Status

All steps done. 162 cases pass (115 + 30 hypothesis small programs, 17 models); one
`#@todo` (s124: recursion deeper than the budget). The hypothesis round found real bugs:
a crash on literal true division, `x.mean(**kw)` reported as `[]`, user `__matmul__` and
user method names taken for tensor ops, `OrderedDict` read as empty.

## Notes / decisions log

* **Runtime oracle** (`tests/torch_oracle.py`, run with
  `uv run --no-project --with torch --with numpy python tests/torch_oracle.py`): every case
  is a runnable program. A driver under `if __name__ == "__main__":` calls every function
  with concrete inputs, and the oracle checks each directive against real PyTorch,
  evaluating dim expressions in the frame. Expectations are still written by hand. The
  oracle checks them against PyTorch, never against rowan. Every case must agree with the
  oracle before the engine work for it starts.
* **Canonical forms in expectations** (what the engine prints in `--golden`; the hover
  uses short letters instead):
  * The dims of an unknown value named `x` (a parameter, an attribute read, …) are
    `x.shape[i]`. Indices are non-negative once the rank is known. While it is unknown,
    dims counted from the end stay negative (`x.shape[-1]`).
  * A rest is a slice of one symbol: `*x.shape[:-1]`, `*x.shape[1:]`, `*x.shape[2:-1]`. A
    parameter that has not been looked into is `[*x.shape]`.
  * Named ints are the program's own terms: `n`, `self.n_embd`, `cfg.d_model`.
    `__init__` arguments use their own names (`d_model`) in every method of the class,
    unless a self attribute holds the same value (`self.d_model`), which is preferred.
  * Two dims not known to be equal broadcast to `max(a, b)`. Two rests broadcast to
    `broadcast(r1, r2)`. An unknown dim is `?`.
  * `x[:, :T]` gives `min(T, x.shape[1])`, and constants are folded. `x[:, 1:]` gives
    `x.shape[1] - 1`: dims are assumed ≥ 1.
  * `x @ y` (and `matmul`) with unknown ranks is the ≥2-D case under assumptions:
    `[*broadcast(x.shape[:-2], y.shape[:-2]), x.shape[-2], y.shape[-1]] if x.dim() >= 2,
    y.dim() >= 2`. The assumption is dropped once a rank is known. If a known rank
    contradicts it, the right case is computed instead.
* **Entry runs**: in a module that imports torch, every function and method is also
  analysed on its own with open parameters (flagged `entry`), even when something calls
  it. `#@shape` without `@L` checks those runs. `@L<n>` checks the runs called from
  line n.
* **Requirements**: `a == b`, `x.dim() == 3`, `x.dim() in (2, 3)`, `x.dim() >= 3`. One is
  recorded only when its condition is undecided, and a `#@requires` line lists all of them
  for that line. View/reshape element counts are never recorded, only checked when
  everything is literal.
* **Cross-function precision** is by tabulation: every function of a torch module is analysed
  per call site (never summarised), and also once on its own (entry run). Fast enough on the
  17 models (the whole torch suite runs in a few seconds).
* **Join atoms**: dims that differ at a join become one atom per set of alternatives (printed
  `?`), stable over loop iterations, so a requirement on it (`h.shape[-1] == d_model` in a
  layer loop) becomes a fact.
* **Unknown non-tensor operands**: in `x * s` with `s` unknown, the result is
  `broadcast(x.shape, s.shape)`, because `s` may be a tensor. Literals, arithmetic on ints
  and dims, and `math.*` results are scalars.

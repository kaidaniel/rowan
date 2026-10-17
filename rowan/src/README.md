# rowan/src

The library (`lib.rs`) and the binary (`main.rs`, `serve.rs`). Three layers, each using only
the ones above it:

1. **Syntax**: parsing and questions about the tree.
2. **Domains**: abstract values, object rows, states, and the persistent maps they live in.
3. **Interpretation**: `interp/` runs the program on those states, calling `models/` for the
   libraries; `interp/report.rs`, `render.rs` and `facts.rs` turn the result into output.

## Files

| file | what |
| --- | --- |
| `lib.rs` | the crate's modules |
| `main.rs` | the command line: options, `run` (analyse one file, print the report / JSON / golden / facts document), `rowan hovers` |
| `serve.rs` | `rowan serve`: the editor's server (analyses on threads of their own, cached per file and its imports, functions answered one at a time when hovered) |
| **syntax** | |
| `frontend.rs` | parsing with ruff, private-name mangling, leaked trees, node positions and text (a node's address is its identity) |
| `scopes.rs` | which names of each function, lambda and class body are locals, cells, free or globals |
| `syntax.rs` | questions the tree alone answers: attribute chains, names used, whether a body yields, returns, jumps, assigns |
| **domains** | |
| `intern.rs` | interned keys (Python literals usable as dict keys), strings, sources and lenses; `IdSet`, the sorted id set every set is |
| `domain.rs` | `Value` (objects + a scalar `KeyVal`), `Row` (an object's keys on a path) and its transfers, `classify` (a read's verdict), `ReqInfo` (a requirement) |
| `source.rs` | `Source` (an input a row is open through) and `Lens` (how the program renamed or removed the input's keys on the way) |
| `layered.rs` | persistent maps (Patricia tries): O(1) copies, joins and diffs that cost what two maps differ in |
| `state.rs` | `State`: what one point on one path knows (names, rows, slots, elements, lengths, requirements, tests passed, tensor facts); `Kind`; the per-thread object marks (merged objects, the shared unknown object, loop views) |
| `interp/` | the interpreter ([`interp/README.md`](interp/README.md)) |
| `models/` | the library models ([`models/README.md`](models/README.md)) |

## How they fit

`main.rs` makes an `interp::Analyzer` for the main file, asks for what it wants recorded (hover
facts, the golden harness's checks), calls `run`, and prints one of `report`'s documents. The
analyzer parses modules through `frontend` (scopes from `scopes`) when they are imported, keeps
its objects in its `Heap`, and threads a `State` through the program: every branch copies the
state (cheap: `layered`'s maps share structure) and joins the copies again (`State::join`,
`Row::join`, `kv_join`). Keys, strings, sources and lenses are `u32` ids (`intern`), so sets
are sorted vectors and joins are merges.

## Reading order

`domain.rs` (`Value`, `KeyVal`, `Row`, `classify`) → `state.rs` (`State`, `join`) →
`interp/mod.rs` → [`interp/README.md`](interp/README.md).

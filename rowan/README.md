# rowan (the engine crate)

The Rust crate behind the VS Code extension: a whole-program abstract interpreter for Python.
It parses a program and its local imports with ruff's parser, runs it on abstract values, and
writes what it found: a text report, JSON lines, the golden harness's document, or the
`--facts` document the editor's hovers are made from.

`AGENTS.md` at the repository's root is the engine's design and the working conventions (tests,
commands, measurements); this folder's READMEs are maps of the code.

## Layout

| path | what |
| --- | --- |
| `Cargo.toml` | one library (`rowan`) and one binary (`rowan`, `src/main.rs`); dependencies: ruff's parser and AST (pinned tag), `smallvec`, `indexmap`, `rustc-hash`, `serde_json`, `mimalloc` |
| `src/` | the library and the binary: see [`src/README.md`](src/README.md) |
| `src/interp/` | the interpreter: see [`src/interp/README.md`](src/interp/README.md) |
| `src/models/` | the library models: see [`src/models/README.md`](src/models/README.md) |
| `py/` | a PyO3 binding, `rowan_py.analyse(source, …)` → the golden document as JSON (not part of the build or CI; builds with `PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1` for Python 3.14; loading the wheel fails on macOS 26) |

## Building

```
cargo build --release          # → target/release/rowan
cargo test --release --lib     # unit tests: CPython-exact values, the regex engine, the tries
```

The release profile keeps debug symbols (`debug = 1`) for profiles. Every local run of the
binary on a real program goes through `../.guard/run` (a memory and time guard; see
`AGENTS.md`).

## The binary

`rowan FILE.py` prints the text report; `--json` the findings as JSON lines; `--golden` the
document `tests/run_golden.py` checks; `--facts` the hover document; `rowan serve` is the
editor's long-running server (JSON lines over stdio); `rowan hovers FILE` times every
function's run on demand (`bench/hovers.py`). The options are in `src/main.rs`'s doc.

## Where to start reading

1. `src/domain.rs` and `src/state.rs`: what a value, a row (an object's keys) and a state are.
2. `src/interp/mod.rs`: the `Analyzer` and its components; then `src/interp/README.md` for the
   order in which a program is run.
3. `src/interp/stmts.rs`, `exprs.rs`, `calls.rs`: the walk itself.
4. A model you care about in `src/models/` (e.g. `pandas.rs`, `torch.rs` with `shape.rs`).

# rowan/src/interp

The abstract interpreter. `Analyzer` (`mod.rs`) walks the program's syntax tree on abstract
`State`s (`../state.rs`): every statement transforms the state, a test splits it in two, every
branch runs and the states join, every call of the program's code runs the callee's body at the
call site. What `AGENTS.md` describes (rows, verdicts, budgets, unknown calls, entry runs) is
implemented here.

## The `Analyzer` and its components

`Analyzer` is a set of components, one field each, each in its own file with its own state; the
walk itself is `impl Analyzer` spread over the other files, one concern per file. Models see
only `heap`, their own `m` (`models::Tables`) and the methods listed in `api.rs`.

| field | file | what it holds |
| --- | --- | --- |
| `program` | `program.rs` | the main file, import paths, the model registry, the modules imported, the classes defined |
| `heap` | `heap.rs` | every object (by allocation site and context) and its payload: the function, class or module it is, a bound method's parts, … |
| `findings` | `findings.rs` | what is recorded for the output: verdicts, notes, summaries, golden row checks, hover facts |
| `names` | `names.rs` | the report's names for unknowns (`row["id"]`, `json.loads(text)`) and unknown call sites |
| `havoc` | `unknown.rs` | unknown code's walks over what it is given, and their memo |
| `effects` | `effects.rs` | what a call that does not run may change, per function code |
| `budget` | `budget.rs` | statements spent per function code, the deadline, the work counters |
| `recursion` | `recursion.rs` | the running activations and recursion's fixpoints |
| `entry` | `entry.rs` | entry runs (functions run with open parameters), runs on demand |
| `gens` | `gens.rs` | generators run at their consumers, per-yield states, `@contextmanager` bodies |
| `unwind` | `tries.rs` | raises in flight, running `try`s and their recorded raise states, loops' breaks |
| `cx` | `context.rs` | the frames, callers' names, the call chain, branch depth, model calls running |
| `mode` | `context.rs` | modes an evaluation sets for the code it calls (guarded reads, narrowing traces, …) |

## How a program runs

1. `entry.rs` `run`: the main module runs (`run_module` → `stmts.rs` `exec_block`).
2. `stmts.rs` `exec_stmt` charges the statement to its function's budget and dispatches:
   assignments bind targets (`funcs.rs` `bind_target`, `store_name`); `if` splits the state
   on its test (`narrow.rs` `split`), runs both branches and joins them; `for` / `while` are
   `loops.rs` (unrolled over known items, else a fixpoint); `try` / `with` are `tries.rs`;
   `class` is `classes.rs`; `def` makes a function object (`funcs.rs` `make_function`);
   `import` runs the module once (`program.rs`).
3. `exprs.rs` `eval` evaluates expressions: names (`funcs.rs` `load_name`), attributes
   (`attrs.rs`), subscripts (`items.rs`), operators (`ops.rs`), comparisons and truth
   (`compare.rs`), comprehensions (`comps.rs`), iteration (`iter.rs`), calls (`calls.rs`).
4. `calls.rs` `call_value` dispatches on what the callee may be: the program's function
   (`call_user`: a frame of its own, the arguments bound, the body run, within the function's
   budget; a recursive call takes the running activation's partial result, `recursion.rs`), a
   class (`construct`), a library function (`call_external`: its model from the registry, or an
   unknown call), unknown code (`unknown.rs` `unknown_call`: what it is given may change),
   several of these (alternatives, one state each). A call past its budget or the deadline does
   not run: `effects.rs` says what it may change, from `flows.rs`'s reading of the code.
5. Objects are made by `alloc.rs` (strong, recent, variant or merged, by context) and dropped
   by `gc.rs` when nothing reaches them (at returns, loop and try exits).
6. Accesses record verdicts and requirements (`access.rs`, `findings.rs` `verdict`): a read of
   a key from an object's row (`../domain.rs` `classify`), a requirement on the input that may
   provide it, under the tests of the function's parameters the path passed (`guards.rs`).
7. After the main module, `entry.rs` runs the functions the program does not call with open
   parameters (and the called ones for their contracts), from the module's end after any of the
   program's code ran (`after_any_code`).
8. `report.rs` (text, JSON, golden), `render.rs` (values as Python text) and `facts.rs` (the
   editor's document) write the result.

## Files

| file | what |
| --- | --- |
| `mod.rs` | `Analyzer`, the module map, the shared bounds (`UNROLL_MAX`, `CHAIN`, `FN_BUDGET`, `DEADLINE_MS`, `ROWAN_LIMITS`) |
| `api.rs` | `Call` (a model's call) and `Args` (a call's arguments), and what the models, the report and the driver may use |
| **components** | |
| `program.rs`, `heap.rs`, `findings.rs`, `names.rs`, `budget.rs`, `recursion.rs`, `entry.rs`, `gens.rs`, `tries.rs`, `context.rs`, `unknown.rs`, `effects.rs` | as in the table above |
| **the walk** | |
| `stmts.rs` | statements: assignments, `if`, `del`, `return`, `raise`, imports, `with`, `match` |
| `loops.rs` | `for` and `while`: unrolled, known prefixes, fixpoints, loop views, requirements per element |
| `classes.rs` | `class` statements and what library bases make of them |
| `exprs.rs` | expressions by kind, displays, subscripts' keys |
| `ops.rs` | operators, f-strings, `%` formatting |
| `compare.rs` | comparisons and truth |
| `comps.rs` | comprehensions |
| `funcs.rs` | function objects and decorators, names, assignment targets and unpacking |
| `iter.rs` | iteration: the items and element of anything iterated |
| `calls.rs` | calls, construction, frames, binding arguments |
| `narrow.rs` | tests split into true and false states, narrowing |
| `guards.rs` | tests of a function's own parameters along a path (`only if` requirements) |
| `access.rs` | what every access shares: guards, requirements, per-receiver runs |
| `items.rs` | item access: reads, stores, deletes, copies of keys, positions, elements |
| `attrs.rs` | attribute access through classes, instances, unknowns |
| `keys.rs` | the key a value stands for (numbers, Enum members, identity-hashed objects), literal key views |
| `alloc.rs`, `gc.rs` | allocation policy; liveness and collection |
| `flows.rs` | what a function's text does with the values it handles (for calls that do not run) |
| **output** | |
| `report.rs` | verdicts combined, access paths of what code reads, the text report, JSON, the golden document |
| `render.rs` | values as Python text, value legends |
| `facts.rs` | the `--facts` document for the editor |

## Reading order

`mod.rs` → `entry.rs` `run` → `stmts.rs` `exec_stmt` → `exprs.rs` `eval` → `calls.rs`
`call_value` / `call_user` → `items.rs` `load_item` / `load_record` (where verdicts and
requirements come from) → `narrow.rs` `split` and `loops.rs` `loop_fixpoint` (how paths
split and join) → `unknown.rs` and `effects.rs` (what rowan assumes of code it does not run).

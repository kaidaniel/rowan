# SUBAGENTS.md: one iteration of improvements

You are a subagent making one iteration of improvements to rowan. Read `README.md` (what it
does for a user) and `AGENTS.md` (how the engine works, the owner's conventions, commands,
tests, debugging switches) first. Everything in `AGENTS.md` applies to you; this file adds
what is specific to an iteration. Your brief may name a task; if it does, that task comes
first, and the objectives below are how it is judged.

## The objectives

All of these should improve, and some conflict. When they do, weigh the trade-off and write
down what you chose and why.

1. **Understandable insights.** Hovers and the CLI report should help a developer understand
   the code: well formatted, every mark explained, not so much that it overwhelms. This is
   easy to put off, so check it against real hovers on real programs in every iteration.
2. **Correct (sound).** What rowan claims must hold for every run of the program. Soundness
   beats everything else here. Also count the golden cases passing with
   `ROWAN_LIMITS=budget=0` (every call skipped: what large programs fall back to once budgets
   run out); raising it is part of this objective.
3. **Informative (precise).** Among sound answers, prefer the ones that say more.
4. **Fast.** An analysis finishes in under 10 seconds; a hover answers in well under one.
5. **Small.** An analysis takes at most 2 GB of memory.
6. **Scalable.** Cost bounded by the design, not only cut by a deadline.
7. **Simple code.** The engine (`rowan/src/*.rs`, `rowan/src/interp/*.rs`; `wc -l`) should
   shrink towards 10k lines. Models (`rowan/src/models/`) don't count, and engine logic must
   not move there. Don't game the count with denser code. A bigger idea that adds code now
   but pays for itself later is welcome; explain the trade-off.
8. **Few knobs.** Every `ROWAN_*` variable and `ROWAN_LIMITS=` key is a cost.
9. **Few open `#@todo`s.** Implement them where a general improvement does, promote the ones
   that already hold (`ROWAN_TODO=check`). They are evidence, not targets: never shape a
   mechanism to make particular golden cases pass.
10. **Modular, readable code.** One concern per component and file, narrow interfaces,
    functions short enough to read at once. Report `Analyzer`'s field count, engine
    functions over 100 lines and the largest engine file before and after.
11. **Installed packages analysable.** Make analysing site-packages possible (`--follow`;
    measured by `bench/sitepackages.py`): above all the plain code a company installs from its
    own packages (business logic, not a framework), which today is unknown code and costs
    the hovers of everything it returns. Libraries and frameworks may stay unknown where
    following them costs too much.

**Small programs first.** Never trade speed, or more than a little precision, on the smaller
programs (stdlib12k, email, rich, click, golden-sized code) for the large ones (pip, timm,
sktime, functional), nor on either for analysing installed packages (objective 11). A real
soundness fix may cost them some speed.

**The held-out corpus is not a target.** `bench/heldout.py` (AGENTS.md, Tests) measures rowan on
real code nobody tunes against. Put its `metrics` (proven %, known facts %, time, memory, cut
runs) and `sound` mismatch count in your before / after table, but never choose work from its
individual modules, sites, hovers or timings, nor change its manifest. Only an unsound claim
its `sound` check finds may become work: shrink it into a golden case written in your own words.
The scoreboard's headline metrics pool the *tracked* set (the eight + the cost set, tuned on
freely); its *combined* headline adds the held-out corpus, and the gap between the two is the
overfitting signal: report all three. **Rotation** is the owner's, about every 20 releases
(AGENTS.md, Tests: `heldout.py rotate --draw`, then `--apply`); never rotate unasked.

**Imprecision is a trade-off,** never "accepted": call a gap "known" or "known to be hard",
name its cost, and list it under Remaining.

**Which libraries matter.** The owner cares most about the builtins, PyTorch, pydantic and
pandas. You may drop another model (sqlite, configparser, argparse, csv, the regex engine,
datetime, FastAPI…) if that makes things materially easier: its calls become unknown calls,
its golden cases go or become `#@todo` with a reason, and your final message says so.

## How to work

1. **Branch.** From an up-to-date `main` (`iter/<topic>`); push it and open a draft pull
   request (`gh pr create --draft`). Never commit to `main`, never merge, never launch
   sub-agents of your own.
2. **Review.** Unless your brief says otherwise, review the code and its design first, the
   main loop most (statements, calls, budgets and skipped calls, joins, liveness, GC). Write
   down what looks wrong, overcomplicated or costly as hypotheses.
3. **Plan.** Choose your items: bugs, performance, refactorings or redesigns (the golden
   suites, the oracle and the snapshots catch most regressions, so a big change is not
   reckless). For each: the objectives it serves and how you will measure it. Prefer items
   that move several at once.
4. **Implement one item at a time,** as `AGENTS.md` says (repro first, then fix, then
   measure; never commit a failing state), one measured commit per item, so the owner can
   keep or revert each step.
5. **Measure.** Locally, `bench/run.py` on all eight programs (up to three runs at a time),
   comparing `--json` verdict counts between `main` and your branch: a new false `missing`
   blocks the merge. Use the `ROWAN_DEBUG=1` `work:` counters for reproducible comparisons.
   Every push runs `bench.yml` (all eight, no deadlines, 10 minute cut, `perf` profiles,
   the `budget=0` golden count): `gh run list --workflow bench.yml --branch <branch>`,
   `gh run view <id>`. The chart at https://kaidaniel.github.io/rowan/dev/bench/ shows `main`.
6. **Iterate** until a good chunk of what you set out to do is done. Don't hand back work you
   could finish.

## On the owner's Mac

* Every local run of rowan, or of a harness that runs it, goes through `.guard/run [-m MB]
  [-t S] cmd…` (16 GB, three runs at a time, `-t 0` for no time limit). Unguarded runs once
  crashed the machine; a guard kill for memory is a finding.
* Leave no processes behind: no orphan runs, no background wait loops.
* Don't touch: the VS Code extension's own `rowan serve`, `../pytorch-playground` (read-only),
  the git stash, `.github/workflows/`.

## Before you hand back

All must pass, on your last commit: `cargo test --release --lib`, golden (and the
`budget=0` count), snapshots (diffs reviewed), the full runtime oracle, the torch oracle,
`cd vscode-rowan && node test.js`, all eight benchmarks run serially before and after, and
CI and Bench green on the pull request. `AGENTS.md` true and under 200 lines.

## Your final message

Also put it in the pull request's description.
* **Commits:** one line each, with its effect.
* **Objectives, before → after** (a table): benchmark time, memory and verdict counts; the held-out
  corpus's metrics and `sound` mismatches; the
  golden and `budget=0` counts; `#@todo`s; engine and model lines; the modularity proxies;
  knobs; what users see.
* **Trade-offs:** what you made less precise or slower, and why.
* **Ideas not pursued:** each with why (unsound, too costly, too much code, no time).
* **Remaining:** what you found but didn't do (bugs, gaps, ideas), each with a repro or a
  pointer. Nothing else records it: the next iteration starts fresh.
* **Not run:** what you did not verify.
* **For the owner:** a plain explanation, for a Python engineer rather than an engine expert,
  of what changed in what they see, with before / after examples.

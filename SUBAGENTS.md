# SUBAGENTS.md: one iteration of improvements

You are a subagent making one iteration of improvements to rowan, an abstract interpreter for
Python written in Rust. Read `README.md` (what it does for a user) and `AGENTS.md` (how the
engine works, the owner's conventions, tests, debugging switches) before anything else. Every
convention in `AGENTS.md` applies to you as well, unless this file says otherwise.

## The objectives

All of these need to improve, and some of them conflict. When they do, weigh the trade-off
and write down what you chose and why.

1. **Understandable insights.** What rowan reports, both in VS Code hovers and in the CLI,
   should help a developer understand the code they are investigating:
   * well formatted;
   * every special symbol explained;
   * not so many insights that they overwhelm.
2. **Correct (sound).** What rowan claims must hold for every run of the program. Soundness
   beats everything else on this list.
3. **Informative (precise).** Among sound answers, prefer the ones that say more.
4. **Fast.** An analysis finishes in under 10 seconds.
5. **Small.** An analysis takes at most 2 GB of memory.
6. **Scalable.** The analysis works on files and programs of any size. Its cost has to be
   bounded by the design, not only cut by a deadline.
7. **Simple code.** The abstract interpreter should be at most 10k lines, and fewer is better.
   * Library models (`rowan/src/models/`) don't count: they can be as long as they need to be.
   * Don't game this number with denser, less readable code. The aim is code that a reader
     can follow.
   * Today the engine outside `models/` is about 28k lines, so there is a long way to go.
     Measure with `wc -l` on `rowan/src/*.rs` and `rowan/src/interp/*.rs`.
8. **Few knobs.** The interpreter should have few configs, limits and switches. Every
   `ROWAN_*` variable, `ROWAN_NO=` refinement and `ROWAN_LIMITS=` key is a cost. A simpler
   design that needs no knob is better than a tuned one.

**Which libraries matter.** The owner cares most about the builtins, PyTorch, pydantic and
pandas. If dropping another model (or accepting imprecision on it) makes things materially
easier, you may decide to do that. Examples are sqlite, configparser, argparse, csv, the regex
engine, datetime and FastAPI. When you drop a model:
* calls into the module become unknown calls, so the results stay sound;
* remove the module's golden cases, or mark them `#@todo` with a reason;
* say so in your final summary.

## How to work

1. **Work on your own branch.** Create a branch from `main` (for example `iter/<topic>`) and
   never commit to `main` directly. Push the branch, and open a draft pull request for it with
   `gh pr create --draft`.
2. **Review first.** Before reading `SUGGESTED_CHANGES.md`, review all of the code and its
   high-level design. Note anything odd or overly complicated, and anything likely to be a
   performance problem (CPU or memory). Write your notes down as hypotheses. The main loop
   (statements, calls, summaries vs per-call-site runs, joins, liveness, GC) deserves the
   most attention.
3. **Compare.** Now read `SUGGESTED_CHANGES.md`, the owner's list of open items. It is
   untracked, in the repository root of the owner's checkout. Compare it with your notes:
   * add what it lacks;
   * drop items your review shows are wrong or already fixed.
4. **Plan.** Write a plan: the items you will work on, in order, each with the objective(s)
   above it serves and how you will measure it. Prefer items that move several objectives at
   once, for example a redesign that bounds work (4, 5, 6) and also removes code and knobs
   (7, 8).
5. **Implement the items one by one.** For each item:
   * **Repro first.** Before fixing a suspected bug or slowdown, write a small program that
     shows it. It becomes a golden case: expectations written by hand from the program
     text, and for cost problems a `#@work`, `#@tabulations`, `#@joins` or `#@visits` bound.
   * **Fix it**, then measure on the big benchmark programs (`bench/`).
   * **Test.** Before every commit:
     * `cargo test`, golden and snapshots must pass. Review the snapshot diffs: changes are
       fine as long as the results are sound.
     * Run the oracle in full when you change models or semantics.
     * Never commit a failing state.
   * **Commit and push**, one commit per item. Messages end with
     `Co-Authored-By: Claude <model> <noreply@anthropic.com>`.
6. **Check the benchmarks.**
   * **Where results appear.** Every push to your pull request runs `.github/workflows/bench.yml`
     on stdlib12k, email and rich, without deadlines, cut at 10 minutes. The workflow
     records wall time, peak memory, work counters, verdict counts and a `perf` profile.
     * Results take about 15 minutes. Read them with `gh run list --workflow bench.yml
       --branch <branch>` and `gh run view <id>` (the job summary has the numbers and the
       top functions).
     * The chart at https://kaidaniel.github.io/rowan/dev/bench/ shows `main` only.
   * **Locally,** run `bench/run.py` (see `AGENTS.md`) with a 2-minute timeout, several runs in
     parallel, in the background.
   * **The goal** is for every benchmarked program to fit within the time and memory limits
     above (10 s, 2 GB). Once they do, add more complicated benchmark programs to
     `bench/` (real packages, larger and more dynamic code) and continue.
7. **Iterate** until you think you have solved a good chunk of the problems you set out to
   solve. Don't stop at the first improvement. Don't hand work back that you could finish.
   Big refactors are welcome: the golden suites and the oracle are your safety net.

## Rules that still hold

* **Python** runs only through `uv run --no-project python`.
* **Golden cases.** Expectations are written by hand before the engine produces them, and are
  never derived from engine output.
* **Soundness.** If the oracle and an expectation disagree, the oracle wins.
* **`SUGGESTED_CHANGES.md`.** Never commit it. Keep it up to date with what you learn:
  remove what you finished, and add what you found.
* **`AGENTS.md`.** Keep it under 200 lines and true at every commit: update its overview
  when the design changes, and its switches when you remove a knob.
* **Measuring.**
  * Compare builds on results (proven / possibly / missing / schema counts of `--json`), not
    only on time.
  * Use `ROWAN_LIMITS=work=N` for reproducible comparisons.
  * Keep runs short and parallel.
  * macOS has no `timeout`: use `perl -e 'alarm N; exec @ARGV' …`.

## Your final message

Summarise what you changed, for the owner:
* **Commits:** each commit, one line each, with its effect.
* **Objectives:**
  * the benchmark numbers before and after (time, memory, verdict counts);
  * the interpreter's line count before and after;
  * the knobs removed;
  * any change to what users see.
* **Trade-offs:** what you dropped or made less precise, and why.
* **Remaining:** what you found but didn't do, and where it is recorded.
* **Not run:** what you did not verify (a test suite not run, a benchmark not finished).

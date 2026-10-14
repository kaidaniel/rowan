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
   * This objective is easy to put off behind the others, so make at least one improvement to
     it in every iteration, and check it against real hovers on real programs.
2. **Correct (sound).** What rowan claims must hold for every run of the program. Soundness
   beats everything else on this list.
   * The golden suites must pass at every commit. Also count how many pass with
     `ROWAN_LIMITS=budget=0`. That setting puts every call past its function's budget, so
     calls are unknown calls (with what `effects.rs` says they may change) instead of runs of
     the body at the call site. Large programs end up in this mode once their budgets run
     out, and many cases fail in it today (the bench workflow reports the count). Raising it
     is part of this objective: a sound and precise treatment of skipped calls is also a route
     to objectives 4-8.
3. **Informative (precise).** Among sound answers, prefer the ones that say more.
4. **Fast.** An analysis finishes in under 10 seconds.
5. **Small.** An analysis takes at most 2 GB of memory.
6. **Scalable.** The analysis works on files and programs of any size. Its cost has to be
   bounded by the design, not only cut by a deadline.
7. **Simple code.** The abstract interpreter should be at most 10k lines, and fewer is better.
   * Library models (`rowan/src/models/`) don't count: they can be as long as they need to be.
   * Don't game this number with denser, less readable code. The aim is code that a reader
     can follow.
   * Today the engine outside `models/` is about 21k lines, so there is a long way to go.
     Measure with `wc -l` on `rowan/src/*.rs` and `rowan/src/interp/*.rs`.
   * Fixes and features tend to add lines, so this one also slips easily. Leave the engine
     smaller than you found it in every iteration: delete, merge or simplify something, not
     just add.
   * The line count is a goal, not a gate. A bigger idea that adds code now but pays for itself
     later (a simpler design that makes other mechanisms deletable, a sound fix that needs new
     machinery) is welcome even if the engine grows this iteration. Don't drop such an idea
     for its size; pursue it, or record it, and explain the trade-off in your final message.
   * Don't move engine logic into `models/` to lower the count: only library behaviour
     belongs there.
8. **Few knobs.** The interpreter should have few configs, limits and switches. Every
   `ROWAN_*` variable and `ROWAN_LIMITS=` key (today only `budget` and `ms`) is a cost. A
   simpler design that needs no knob is better than a tuned one.
9. **Few open `#@todo`s.** A `#@todo` is an expectation the engine doesn't meet yet
   (`run_golden.py` reports how many it skips). Fewer is better: implement them where you
   can, and promote the ones that already hold (`ROWAN_TODO=check`). Adding an honest
   `#@todo` is fine when a new case shows something the engine can't do yet; weigh this
   count like the other objectives, not as a rule of its own.

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
   (statements, calls: per-call-site runs, budgets and skipped calls, joins, liveness, GC)
   deserves the most attention.
3. **Compare.** Now read `SUGGESTED_CHANGES.md`, the owner's list of open items. It is
   untracked, in the repository root of the owner's checkout. Compare it with your notes:
   * add what it lacks;
   * drop items your review shows are wrong or already fixed.
4. **Plan.** You are free to decide what kind of work to do: look for bugs to fix, make
   iterative performance improvements, or do large-scale refactorings and redesigns. The test
   suite (about a thousand golden programs, the runtime oracle, snapshots) catches many
   regressions, so a big change is not reckless. Write a plan: the items you will work on,
   in order, each with the objective(s) above it serves and how you will measure it. Prefer items that move several objectives at
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
     on all eight benchmark programs (stdlib12k, email, rich, click; the frontier pip, timm,
     sktime, functional), without deadlines, cut at 10 minutes. The workflow records wall
     time, peak memory, work counters, verdict counts and a `perf` profile, and the golden
     count with `budget=0`.
     * Results take about 5-10 minutes. Read them with `gh run list --workflow bench.yml
       --branch <branch>` and `gh run view <id>` (the job summary has the numbers and the
       top functions).
     * The chart at https://kaidaniel.github.io/rowan/dev/bench/ shows `main` only.
   * **Locally,** run `bench/run.py` (see `AGENTS.md`), up to three runs in parallel, in the
     background. Compare `--json` verdict counts between `main` and your branch on all eight
     programs: a new false `missing` anywhere blocks the merge.
   * **The goal** is for every benchmarked program to fit within the time and memory limits
     above (10 s, 2 GB). The four smaller programs matter first: never trade their speed, or
     more than a little of their precision, for the frontier ones. Savings found on the large
     programs are welcome when they generalise. Once all fit, add more complicated benchmark
     programs to `bench/` (real packages, larger and more dynamic code) and continue.
7. **Iterate** until you think you have solved a good chunk of the problems you set out to
   solve. Don't stop at the first improvement. Don't hand work back that you could finish.
   Big refactors are welcome: the golden suites and the oracle are your safety net.

## Rules that still hold

* **Python** runs only through `uv run --no-project python`.
* **Golden cases.** Expectations are written by hand before the engine produces them, and are
  never derived from engine output.
* **Soundness.** If the oracle and an expectation disagree, the oracle wins.
* **`SUGGESTED_CHANGES.md`.** Never commit it. It lists only avenues not yet explored: open
  bugs, ideas and gaps, each with a repro or a pointer, grouped by topic (not by iteration).
  It is not a changelog (the PRs are): no "done in PR #N", no measurements of past work.
  Remove what you finished or ruled out (a ruled-out idea goes in your final message's "Ideas
  not pursued", and stays in the file only as one line saying why, so nobody retries it
  blindly), and add what you found under its topic.
* **`AGENTS.md`.** Keep it under 200 lines and true at every commit: update its overview
  when the design changes, and its switches when you remove a knob.
* **Measuring.**
  * Compare builds on results (proven / possibly / missing / schema counts of `--json`), not
    only on time.
  * For reproducible comparisons use the deterministic counters of `ROWAN_DEBUG=1` (the final
    `work:` line), not only wall time.
  * On the owner's Mac, every local run of rowan (and of a harness that spawns it) goes
    through `.guard/run [-m MB] [-t S] cmd…` (16 GB per run and in total, three runs at a
    time, `-t 0` for no time limit). Leave no orphan processes. Unguarded runs once crashed
    the machine; a guard kill for memory is a finding.

## Your final message

Summarise what you changed, for the owner:
* **Commits:** each commit, one line each, with its effect.
* **Objectives:**
  * the benchmark numbers before and after (time, memory, verdict counts);
  * the golden pass count with `ROWAN_LIMITS=budget=0`, before and after;
  * the number of `#@todo` expectations, before and after;
  * the engine's and the models' line counts before and after;
  * the knobs removed;
  * any change to what users see.
* **Trade-offs:** what you dropped or made less precise, and why.
* **Ideas not pursued:** ideas you considered but didn't pursue or cut back, each with why
  (unsound, too costly, too much code, no time). Include the ones you dropped because of their
  size.
* **Remaining:** what you found but didn't do, and where it is recorded.
* **Not run:** what you did not verify (a test suite not run, a benchmark not finished).

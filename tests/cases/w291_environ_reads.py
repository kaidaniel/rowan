"""Hypothesis: `os.environ[k]` is a requirement, `os.environ.get` and a read guarded by `in`
are not.

Expectations were written from the program text before running the engine."""
import os

home = os.environ["HOME"]                    #@ input HOME
user = os.environ.get("USER", "x")           #@ none USER
if "CI" in os.environ:
    ci = os.environ["CI"]                    #@ ok CI
#@schema os.environ: always={HOME} only={HOME}

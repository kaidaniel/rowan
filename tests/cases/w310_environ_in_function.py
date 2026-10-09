"""Hypothesis: `os.environ` reads inside a called function are requirements like direct ones.

Expectations were written from the program text before running the engine."""
import os


def get_home():
    return os.environ["HOME"]


a = get_home()
b = get_home()
os.environ["PATH"]                           #@ input PATH
#@schema os.environ: always={HOME,PATH}

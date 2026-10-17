"""Hypothesis: a function the program only hands to code rowan cannot see (a callback given to an
unknown library, one stored where only unknown code reads it) is never run by the program, so it
gets a run of its own with open parameters and its verdicts: a key its module never sets is
missing there, and what it reads of its parameter is needed of it. Unknown code may call it, but
that does not count as the program calling it.

Expectations were written from the program text before running the engine."""
import somelib

CONFIG = {"a": 1}
HOOKS = somelib.Registry()


def on_event(evt):
    return CONFIG["b"] + evt["x"]               #@ missing b ; input x
#@schema param on_event.evt: always={x} only={}


def on_close(evt):
    return CONFIG["a"] + evt["y"]               #@ ok a ; input y


somelib.register(on_event)
HOOKS.close = on_close
somelib.run(HOOKS)

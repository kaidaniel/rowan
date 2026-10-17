"""A fake installed module whose import needs more work than an entry gets (n97_allowance.py)."""
from heavylib import heavy

STATE = {}
heavy(STATE)


def get():
    return STATE["n"]

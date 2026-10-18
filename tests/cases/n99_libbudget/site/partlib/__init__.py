"""A fake installed package whose import needs more work than an entry gets (n99_lib_budgets.py):
it registers one name, builds a table with 256 calls, registers another name."""
from partlib.core import REGISTRY, cheap, expensive, register, tag_all

register("early", 1)
TABLE = expensive()
register("late", 2)
NAME = "partlib"

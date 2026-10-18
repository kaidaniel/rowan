"""Hypothesis: an unknown named by the expression it comes from (`retries - 1`), joined with a
literal (`max(1, retries - 1)`, `0 if flag else y`), is no longer that expression: its hover is
`1 | int(?)`, not `limit = retries - 1` (which reads as an equation and hides the literal). Values
stay the same: comparisons with the expression are undecided. vscode-rowan/test.js checks the
hovers.

Expectations were written from the program text before running the engine."""
import sys

retries = int(sys.argv[1])
y = retries - 1
limit = max(1, y)                                    #@value limit: types={int}
same = "same" if limit == y else "diff"              #@value same: exactly={"same", "diff"}
t = 0 if len(sys.argv) > 2 else y                    #@value t: types={int}
print(limit, same, t)

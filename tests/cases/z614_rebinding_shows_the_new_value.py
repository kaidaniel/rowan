"""Hypothesis: a line that rebinds a name from its own value (`x = x + 1`, `name = name.strip()`)
holds the new value after it: its hover shows that (`x = 2`), not the old and the new as
alternatives (`x = 1 | 2`, whose note says "depending on the path"). vscode-rowan/test.js checks
the hovers.

Expectations were written from the program text before running the engine."""
x = 1
x = x + 1                                    #@value x: exactly={2}
name = "  Ada "
name = name.strip()                          #@value name: exactly={"Ada"}
items = [1, 2]
items = sorted(items) + [3]                  # (a list: its length is checked below)
n = len(items)                               #@value n: exactly={3}

"""Hypothesis: tuple keys (written with or without parentheses) are literal keys like any
other.

Expectations were written from the program text before running the engine."""

d = {}
d[(1, 2)] = "a"
d[2, 1] = "b"
x = d[(1, 2)]                                #@ ok (1, 2)
d[(3, 3)]                                    #@ missing (3, 3)
grid = {}
for i, j in [(0, 0), (0, 1)]:
    grid[i, j] = 0
grid[0, 1]                                   #@ ok (0, 1)
grid[1, 1]                                   #@ missing (1, 1)

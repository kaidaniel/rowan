# zip(*rows) over a number of rows not known: each tuple has as many items as there are rows,
# a number not known (not two: the rows' own items say nothing of how many rows there are).
import sys

n = len(sys.argv) + 2
cols = [[1, 2] for _ in range(n)]
sizes = [len(row) for row in zip(*cols)]
k = sizes[0]
#@value k: unknown
count = 0
for row in zip(*cols):
    a, b, c = row
    count += 1
#@value count: includes={2}
pairs = list(zip(*[[1, 2], [3, 4]]))
q = pairs[1][0]
#@value q: exactly={2}

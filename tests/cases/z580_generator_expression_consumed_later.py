# A generator expression runs as it is consumed: its first iterable is taken when it is made,
# but items appended to that list before it is consumed are still iterated.
xs = [1, 2]
g = (w * 2 for w in xs)
xs.append(-1)
last = list(g)[-1]
#@value last: exactly={-2}
h = (w * 2 for w in xs)
first = next(h)                              #@value first: exactly={2}

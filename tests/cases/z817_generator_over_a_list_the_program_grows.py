"""A generator expression over a list takes the list's items as it runs, from where it stands:
items the program appends after its first item are among the later ones."""
xs = [1, 2, 3]
it = (v * 2 for v in xs)
first = next(it)
#@value first: includes={2}
#@todo value first: exactly={2}  (run again with the list any at every yield, its first item too)
xs.append(4)
total = sum(it)
#@value total: includes={18}
#@todo value total: exactly={18}  (the generator runs again with the list changed by unknown code at each yield: an int)

ys = [1, 2, 3]
jt = (v for v in ys)
found = 2 in jt
#@value found: includes={True}
#@todo value found: exactly={True}  (as first)
ys.append(4)
after = list(jt)
k = len(after)
#@value k: includes={2}
#@todo value k: exactly={2}  (as total)

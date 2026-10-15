# A comprehension over items of unknown number makes one object per item: what a store into
# the first one does says nothing of the others (the one pass the comprehension runs made its
# objects as single ones, so a store into `xs[0]` was taken as a store into every item).
import os


class Foo:
    pass


items = os.environ.get("ROWAN_Z510_ITEMS", "p").split(",")

xs = [Foo() for _ in items]
xs[0].a = 1
for x in xs:
    print(x.a)  #@ possibly a

rows = [{} for _ in items]
rows[0]["n"] = 1
for r in rows:
    print(r["n"])  #@ possibly n

# known items: each iteration makes its own object, positions known
ys = [Foo() for _ in range(3)]
ys[0].b = 1
print(ys[0].b)  #@ ok b

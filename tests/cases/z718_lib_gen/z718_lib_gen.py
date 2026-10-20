"""A generator a followed package's code makes runs its body where the program consumes it, as
an entry into the package's code, also when another entry made it (`made()` returns `rows()`'s
generator): within the library's share (`#@limits budget=64`: 64 × 4 × 64 = 16,384 of work).
`rows()`'s body needs 65,536 steps: past the share its consumption is undone and its items are
unknown, as without following; then no package code runs (`rows()` itself: a call that does not
run, `small()` too); the program's own facts stay exact.

Expectations were written from the program text before running the engine (`n_items` was
`items: unknown`, wrong: small() is cheap, its items are known)."""
#@argv --follow genlib --site-packages site
#@limits budget=64
#@work <= 20000
import genlib

n_items = len(list(genlib.small()))          #@value n_items: exactly={2}
first = next(genlib.small())                 #@value first: includes={"a"}
total = 0
for n in genlib.made():
    total += n
#@value total: includes={65536}
again = 0
for n in genlib.rows():
    again += n
#@value again: includes={65536}
late = next(genlib.small())                  #@value late: includes={"a"}
mine = {"k": 1}
k = mine["k"]                                #@value k: exactly={1}

# A generator whose run is past its function's budget is unknown code: its items are unknown
# values (any number of them), not none. (gen_rows's second call is past the budget; its run
# happens where `yield from` consumes it.)
#@limits budget=2


def gen_rows(n):
    for i in range(n):
        yield {"i": i}


first = list(gen_rows(2))


def chain():
    yield {"a": 1}
    yield from gen_rows(3)


for c in chain():
    c["a"]                                       #@ input a
    #@todo possibly a  (gen_rows's items are not known past its budget)

items = list(chain())
n = len(items)
#@value n: unknown
#@todo value n: exactly={4}  (gen_rows's items are not known past its budget)

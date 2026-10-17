"""Hypothesis: `left + right` of two values rowan cannot see keeps what each side's elements
are: iterating the result reads the elements of either, so a key read from an element is
needed of both sides' elements; a slice of it, or a concatenation with a third, too. A sum
that is a number stays an unknown.

Expectations were written from the program text before running the engine."""


def ids(left, right):
    return [r["id"] for r in left + right]
#@schema param ids.left['*']: always={id} only={}
#@schema param ids.right['*']: always={id} only={}


def tail_names(a, b, c):
    out = []
    for p in (a + b)[1:] + c:
        out.append(p["name"])
    return out
#@schema param tail_names.a['*']: always={name} only={}
#@schema param tail_names.b['*']: always={name} only={}
#@schema param tail_names.c['*']: always={name} only={}


def total(x, y):
    s = x + y
    #@general s: unknown
    return s


if __name__ == "__main__":
    assert ids([{"id": 1}], [{"id": 2}]) == [1, 2]
    assert tail_names([{"name": "a"}], [{"name": "b"}], [{"name": "c"}]) == ["b", "c"]
    assert total(1, 2) == 3

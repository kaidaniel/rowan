"""Hypothesis: a parameter annotated with a builtin container of builtin types (`list[dict[str,
int]]`) is, for any call, a container of values of those types: an element read of it is of the
element's type, a value read of a dict of the value's type, a tuple's position of its own type.

Expectations were written from the program text before running the engine."""


def total(rows: list[dict[str, int]]) -> int:
    out = 0
    for r in rows:
        v = r["n"]
        #@general v: types={int}
        out += v
    return out


def first_key(m: dict[str, list[bytes]]):  # (not float: an int passes for a float)
    for k in m:
        key = k
        #@todo general key: types={str}  (a key and any key's value share one unknown, `m[*]`)
        xs = m[k]
        for x in xs:
            item = x
            #@todo general item: types={bytes}  (the same: `m[k]` is `m[*]`)
            return key, item
    return None


def pairs(ps: tuple[int, str]):
    a = ps[0]
    b = ps[1]
    #@general a: types={int}
    #@general b: types={str}
    return a, b


if __name__ == "__main__":
    assert total([{"n": 1}, {"n": 2}]) == 3
    assert first_key({"a": [b"x"]}) == ("a", b"x")
    assert pairs((1, "x")) == (1, "x")
    assert values_of({"a": [b"y"]}) == b"y"

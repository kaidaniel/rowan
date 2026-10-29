"""For any call, the keys of a parameter annotated `dict[str, V]` are strs, also through its
views (`d.items()`, `d.keys()`); a key of another type the function stores (`d[5] = …`) stands
beside them, and the strs keep their type (`5 | str(?)`). Unknown code given the dict may put
any key there. (Before, a view's keys were unknowns of no type, and a non-str key stored made
every key an unknown of no type, `5 | ?`.)

Expectations were written from the program text before running the engine."""


def views(d: dict[str, int]):
    for k, v in d.items():
        a = k
        #@general a: types={str}
    for k2 in d.keys():
        b = k2
        #@general b: types={str}
    return len(d)


def mixed(d: dict[str, int]):
    d[5] = "t"
    for key in d:
        kk = key
        #@general kk: shows={5} types={str}
    for k, v in d.items():
        kj = k
        #@general kj: shows={5} types={str}


def given(d: dict[str, int], f):
    f(d)
    for key in d:
        g = key
        #@general g: untyped


def put_key(d):
    d[(1, 2)] = 0


views({"a": 1})
mixed({"b": 2})
given({"c": 3}, put_key)

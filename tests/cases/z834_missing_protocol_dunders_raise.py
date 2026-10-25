"""An instance of a class of the program's alone without `__setitem__`, `__delitem__`,
`__enter__` / `__exit__`, or any of `__contains__` / `__iter__` / `__getitem__`: `x[0] = 1`,
`del x[0]`, `with x:` and `1 in x` raise TypeError."""


class Plain:
    pass


def attempt(kind):
    p = Plain()
    try:
        if kind == "set":
            p[0] = 1
        elif kind == "del":
            del p[0]
        elif kind == "with":
            with p:
                pass
        else:
            return 1 in p
        return "ok"
    except TypeError:
        return "TypeError"


s = attempt("set")
#@value s: exactly={"TypeError"}
d = attempt("del")
#@value d: exactly={"TypeError"}
w = attempt("with")
#@value w: exactly={"TypeError"}
i = attempt("in")
#@value i: exactly={"TypeError"}

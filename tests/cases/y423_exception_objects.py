"""Hypothesis: exception objects carry what Python gives them: `KeyError("k")` is an object with
args, `type(e).__name__` names its class, `raise X from Y` sets X.__cause__, a user Exception
subclass gets args from `super().__init__(msg)` (or from its arguments without an __init__), so
str(e) is the message, also when the exception crosses a function call into a handler.

Expectations were written from the program text before running the engine."""


class ParseError(Exception):
    def __init__(self, field):
        super().__init__(field + ": missing")
        self.field = field


class Plain(Exception):
    pass


def need(rec, k):
    if k not in rec:
        raise ParseError(k)
    return rec[k]


if __name__ == "__main__":
    k = KeyError("k")
    kn = type(k).__name__                        #@value kn: exactly={"KeyError"}
    ka = k.args[0]                               #@value ka: exactly={"k"}
    try:
        raise ValueError("x") from k
    except ValueError as e:
        cause = type(e.__cause__).__name__       #@value cause: exactly={"KeyError"}
        sv = str(e)                              #@value sv: exactly={"x"}
    try:
        need({}, "n")
        msg = "none"
    except ParseError as pe:
        msg = str(pe)
        fld = pe.field                           #@value fld: exactly={"n"}
    _ = 0                                        #@value msg: exactly={"n: missing"}
    p = Plain("m")
    ps = str(p)                                  #@value ps: exactly={"m"}

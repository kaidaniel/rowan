"""Hypothesis: an exception caught is the one raised, with its arguments and the attributes its
`__init__` set; `raise X from Y` sets X's `__cause__` to Y. (Hovers write them as Python does:
`AppError("too big")`, `Coded("negative", __cause__=KeyError("k"), code=400)`.)

Expectations were written from the program text before running the engine."""


class AppError(Exception):
    pass


class Coded(Exception):
    def __init__(self, code, msg):
        super().__init__(msg)
        self.code = code


def risky(x):
    if x > 1:
        raise AppError("too big")
    if x < 0:
        raise Coded(400, "negative") from KeyError("k")
    return x


def run(x):
    try:
        return risky(x)
    except AppError as e:
        err = e
        msg = err.args[0]
        #@value msg: exactly={"too big"}
        return err
    except Coded as c:
        coded = c
        code = coded.code  #@ ok code
        #@value code: exactly={400}
        cause = coded.__cause__
        first = cause.args[0]
        #@value first: exactly={"k"}
        return coded


run(2)
run(-1)
run(1)

# click's `augment_usage_errors`: a `@contextmanager` handler of the exceptions a parameter's
# conversion raises, run for each parameter of a list (a loop). Every BadParameter has `ctx` and
# `param` once built: `__init__` (past its budget here, so a call that does not run) sets them
# through `super()`. An exception an earlier round left in an outer try is another path's; an
# instance whose `__init__` a raise leaves (also inside the with body) is garbage there.
#@limits budget=5
import sys
from contextlib import contextmanager


class ClickException(Exception):
    def __init__(self, message):
        super().__init__(message)
        self.message = message


class UsageError(ClickException):
    def __init__(self, message, ctx=None):
        super().__init__(message)
        self.ctx = ctx
        self.cmd = self.ctx.command if self.ctx else None


class BadParameter(UsageError):
    def __init__(self, message, ctx=None, param=None, param_hint=None):
        super().__init__(message, ctx)
        self.param = param
        self.param_hint = param_hint


class MissingParameter(BadParameter):
    def __init__(self, message=None, ctx=None, param=None, param_hint=None, param_type=None):
        super().__init__(message or "", ctx, param, param_hint)
        self.param_type = param_type


@contextmanager
def augment(ctx, param=None):
    try:
        yield
    except BadParameter as e:
        if e.ctx is None:
            #@ ok ctx
            e.ctx = ctx
        if param is not None and e.param is None:
            #@ ok param
            e.param = param
        raise
    except UsageError as e:
        if e.ctx is None:
            e.ctx = ctx
        raise


class Param:
    def __init__(self, nargs, required):
        self.nargs = nargs
        self.required = required

    def convert(self, ctx, value):
        if len(value) != self.nargs:
            raise BadParameter("bad", ctx=ctx, param=self)
        return value

    def process(self, ctx, value):
        value = self.convert(ctx, value)
        if self.required and not value:
            raise MissingParameter(ctx=ctx, param=self)
        return value

    def handle(self, ctx, value):
        with augment(ctx, param=self):
            return self.process(ctx, value)


class Ctx:
    command = "c"


def run(params, values):
    out = []
    ctx = Ctx()
    for p in params:
        v = values.pop()
        out.append(p.handle(ctx, v))
    return out


def main(argv):
    try:
        return run([Param(int(a), a == "r") for a in argv], argv)
    except Exception:
        return None


main(sys.argv[1:])
main([])
main(["2"])

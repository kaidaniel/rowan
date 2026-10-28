"""A parameter annotated with the program's class may be an instance of any of its subclasses,
also one that overrides nothing: for any call, `isinstance(o, Sub)` and `type(o)` are not
decided. (Before, the run for any call took `o: Base` for an instance of Base itself when the
subclass added nothing: `isinstance(o, Sub)` was False and `type(o).__name__` was "Base", also
at the call site of a call that does not run.)

Expectations were written from the program text before running the engine."""
import sys


class Base:
    v = 1

    def __init__(self):
        self.a = 1


class Sub(Base):
    pass


def kind(o: Base):
    is_sub = isinstance(o, Sub)
    #@general is_sub: types={bool}  # (an undecided isinstance is `bool(?)`, not True | False)
    name = type(o).__name__
    #@general name: exactly={"Base", "Sub"}
    cls_name = o.__class__.__name__
    #@general cls_name: exactly={"Base", "Sub"}
    exact = type(o) is Base
    #@general exact: types={bool}  # (as is_sub)
    return "sub" if is_sub else "base"


o = Base() if len(sys.argv) > 2 else Sub()
r = kind(o)
#@value r: exactly={"base", "sub"}

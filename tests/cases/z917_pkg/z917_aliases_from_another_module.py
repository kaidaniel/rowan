"""For any call, a parameter annotated with an alias another module of the program binds (`from
kinds import Justify`) is what the alias names, as one bound in its own module is (z912): a
`Literal`'s values, an int or None, a class's instance or a str; the alias's names are those of
the module it is written in.

Expectations were written from the program text before running the engine."""
from kinds import Drawable, Justify, Shape, Size


def align(how: Justify, width: Size, d: Drawable):
    h = how
    #@general h: exactly={'left', 'right', 'center'}
    w = width
    #@general w: shows={None} types={int}
    if isinstance(d, Shape):
        n = d.sides
        #@general n: includes={4}
    else:
        s = d
        #@general s: types={str}
    return h


align("left", 3, Shape())
align("center", None, "x")

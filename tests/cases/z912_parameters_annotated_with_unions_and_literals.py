"""For any call, a parameter is what its annotation allows, member by member, as a field's is: a
`Literal`'s values exactly, `int | str` an int or a str (each told apart by `isinstance`), a
union of the program's class and a builtin type an instance or that type, `*args: int` ints,
`**kw: str` str values under str keys; an alias the module binds once (`Mode = Literal[...]`)
stands for what it names. (Before, only one builtin type, one class, or either with None was
taken: these were unknowns.)

Expectations were written from the program text before running the engine."""
from typing import Literal, Optional, Union

Mode = Literal["r", "w"]


class Shape:
    def __init__(self):
        self.sides = 4

    def area(self):
        return 1


Drawable = Union[Shape, str]


def open_file(mode: Literal["r", "w", "a"]):
    m = mode
    #@general m: exactly={'r', 'w', 'a'}
    return m


def open_alias(mode: Mode):
    m2 = mode
    #@general m2: exactly={'r', 'w'}
    return m2


def maybe(flag: Optional[Literal[1, 2]]):
    f = flag
    #@general f: exactly={1, 2, None}
    return f


def either(x: int | str):
    v = x
    #@general v: types={int, str}
    if isinstance(x, str):
        s = x
        #@general s: types={str}
    else:
        i = x
        #@general i: types={int}
    return v


def shape_or_name(sh: Shape | str):
    if isinstance(sh, str):
        n = sh
        #@general n: types={str}
        return n
    a = sh.area()
    #@general a: exactly={1}
    return a


def draw(d: Drawable):
    if isinstance(d, Shape):
        sides = d.sides
        #@general sides: includes={4}
        return sides
    return d


def stars(*args: int, **kw: str):
    for a in args:
        x = a
        #@general x: types={int}
    for k, val in kw.items():
        y = val
        #@general y: types={str}
        kk = k
        #@general kk: types={str}
        # (a `**kw` dict's keys are strs: Python takes no other keyword names)


open_file("a")
open_alias("w")
maybe(None)
maybe(2)
either(3)
either("t")
shape_or_name(Shape())
shape_or_name("s")
draw(Shape())
draw("d")
stars(1, 2, p="q")

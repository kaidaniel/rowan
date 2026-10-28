"""Unpacking a value that may be an instance of the program's class without `__iter__` /
`__getitem__` raises TypeError on that instance's path only: the other values beside it (an
unknown, a list, a str for any call) are unpacked and the code after goes on. Subscripting an
Enum member raises TypeError (an Enum's members are no containers); an instance of a class with
another library base may raise anything there. (Before, the TypeError of the one instance ended
every path: the lines after the unpacking were taken for unreachable for any call; and an Enum
member's subscript raised nothing.)

Expectations were written from the program text before running the engine."""
import enum
import json
from typing import Union


class Ob:
    v = 1


def mixed(s, cond):
    x = Ob() if cond else json.loads(s)
    try:
        a, b = x
        r = b
        #@general r: unknown
    except TypeError:
        r = "not iterable"
        #@general r: exactly={'not iterable'}
    return r


def annotated(x: Union[str, Ob]):
    try:
        a, b = x
        out = b
        #@general out: types={str}
    except TypeError:
        out = "no"
    except ValueError:
        out = "length"
    return out


class Color(enum.Enum):
    RED = 1
    BLUE = 2


def member_item(c: Color):
    try:
        v = c["k"]
    except TypeError:
        v = "not subscriptable"
    #@general v: exactly={'not subscriptable'}
    return v


mixed("[1, 2]", False)
mixed("", True)
annotated("ab")
annotated(Ob())
member_item(Color.RED)

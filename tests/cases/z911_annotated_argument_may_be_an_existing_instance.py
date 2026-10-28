"""For any call, a parameter annotated with a class (and a method's receiver, and a field an
annotation gives a class) may be any instance of that class: also one the program made itself
(a module-level default, a registry's key, a sentinel), and the very object another parameter
is. Identity says nothing about it then: `t is DEFAULT`, `t == DEFAULT` (no `__eq__`: identity),
`t in KNOWN` and `LABELS[t]` may all hold. An Enum-annotated parameter is one of its members.
(Before, the instance a run for any call made was taken for a new object: those branches were
claimed unreachable, and `LABELS[t]` was reported to fail on every path.)

Expectations were written from the program text before running the engine."""
import enum
from dataclasses import dataclass


class Token:
    def __init__(self, name):
        self.name = name


DEFAULT = Token("default")
LABELS = {DEFAULT: "the default"}
KNOWN = {DEFAULT}
ORDER = [DEFAULT]


def label(t: Token):
    if t is DEFAULT:
        hit = 1
        #@general hit: exactly={1}
    if t == DEFAULT:
        eq = 2
        #@general eq: exactly={2}
    if t in KNOWN:
        known = 3
        #@general known: exactly={3}
    if t in ORDER:
        ordered = 4
        #@general ordered: exactly={4}
    text = LABELS[t]
    #@general text: exactly={'the default'}
    return text


def same(a: Token, b: Token):
    if a is b:
        s = 5
        #@general s: exactly={5}
    return a


class Node:
    def __init__(self):
        self.parent = None

    def is_root(self):
        if self is ROOT:
            r = 6
            #@general r: exactly={6}
            return True
        return False


ROOT = Node()


@dataclass
class Box:
    tok: Token


def unbox(b: Box):
    if b.tok is DEFAULT:
        q = 7
        #@general q: exactly={7}


class Color(enum.Enum):
    RED = 1
    BLUE = 2


def paint(c: Color):
    if c is Color.RED:
        p = 8
        #@general p: exactly={8}
    n = c.name
    #@general n: exactly={'RED', 'BLUE'}
    return n


label(DEFAULT)
same(DEFAULT, DEFAULT)
ROOT.is_root()
unbox(Box(DEFAULT))
paint(Color.RED)
paint(Color.BLUE)

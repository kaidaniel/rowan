"""Hypothesis: comparisons, str-method reads of values not known, and `%` formatting may raise,
and a try sees it. An ordering (`<`) of an input's values may raise TypeError (an int against a
str); two ints, or two strs, cannot; `1 < "a"` surely raises, `==` never does. A method read
like `x.strip` of an input's value may raise AttributeError (an int has none); of a str it
cannot. `"%d" % x` may raise TypeError when x may be no number; `"%d" % "many"` and too few
arguments surely raise; `"%s" % x` of one argument never does. Before, every one of these
handlers was dead. A literal's attribute its type lacks (`(5).strip`) surely raises.

Expectations were written from the program text before running the engine."""
import json
import os

DATA = json.loads(os.environ.get("Z773_DATA", '{"x": 6, "y": 3, "s": "a"}'))


def ordered(a, b):
    try:
        r = a < b
        how = "ok"
    except TypeError:
        how = "type"
    return how
    #@value how: exactly={"ok", "type"}


def typed_ordered(a: int, b: int):
    try:
        r = a <= b
        how = "ok"
    except TypeError:
        how = "type"
    return how
    #@general how: exactly={"ok"}


def literal_ordered():
    try:
        r = 1 < "a"
        how = "ok"
    except TypeError:
        how = "type"
    return how
    #@value how: exactly={"type"}


def equal(a, b):
    try:
        r = a == b
        how = "ok"
    except TypeError:
        how = "type"
    return how
    #@value how: exactly={"ok"}


def stripped(x):
    try:
        f = x.strip
        how = "ok"
    except AttributeError:
        how = "attr"
    return how
    #@value how: exactly={"ok", "attr"}


def called_strip(x):
    try:
        v = x.strip()
        how = "ok"
    except AttributeError:
        how = "attr"
    return how
    #@value how: exactly={"ok", "attr"}


def typed_strip(x: str):
    try:
        v = x.strip()
        how = "ok"
    except AttributeError:
        how = "attr"
    return how
    #@general how: exactly={"ok"}


def literal_attr(c):
    v = 5 if c else 2.5
    try:
        f = v.strip
        how = "ok"
    except AttributeError:
        how = "attr"
    return how
    #@value how: exactly={"attr"}


def literal_some_attr(c):
    v = 5 if c else "five"
    try:
        f = v.upper
        how = "ok"
    except AttributeError:
        how = "attr"
    return how
    #@value how: exactly={"ok", "attr"}


def formatted(n):
    try:
        t = "%d items" % n
        how = "ok"
    except TypeError:
        how = "type"
    return how
    #@value how: exactly={"ok", "type"}


def formatted_lit():
    try:
        t = "%d items" % "many"
        how = "ok"
    except TypeError:
        how = "type"
    return how
    #@value how: exactly={"type"}


def too_few(a):
    try:
        t = "%s and %s" % (a,)
        how = "ok"
    except TypeError:
        how = "type"
    return how
    #@value how: exactly={"type"}


def as_text(a):
    try:
        t = "<%s>" % (a,)
        how = "ok"
    except TypeError:
        how = "type"
    return how
    #@value how: exactly={"ok"}


ordered(DATA["x"], DATA["s"])
typed_ordered(1, 2)
literal_ordered()
equal(DATA["x"], DATA["s"])
stripped(DATA["x"])
called_strip(DATA["s"])
typed_strip("  a ")
literal_attr(DATA["x"])
literal_some_attr(DATA["x"])
formatted(DATA["s"])
formatted_lit()
too_few(DATA["x"])
as_text(str(DATA["y"]))

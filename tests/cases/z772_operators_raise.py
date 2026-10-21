"""Hypothesis: an operator on values not known may raise, and a try sees it: a TypeError where
the operands' types do not surely fit (`a + b` of an input's values; not two ints), a
ZeroDivisionError for `/`, `//` and `%` where the divisor may be zero (not by a literal 2), a
ValueError for a shift by a count that may be negative. Two literals raise as Python raises
(`1 / 0`: surely). Before, operators raised nothing: every handler below was dead.

Expectations were written from the program text before running the engine."""
import json
import os

DATA = json.loads(os.environ.get("Z772_DATA", '{"x": 6, "y": 3, "s": "a", "k": 2}'))


def ratio(a, b):
    try:
        r = a / b
        how = "ok"
    except ZeroDivisionError:
        how = "zero"
    return how
    #@value how: exactly={"ok", "zero"}


def typed_ratio(a: int, b: int):
    try:
        r = a // b
        how = "ok"
    except ZeroDivisionError:
        how = "zero"
    return how
    #@general how: exactly={"ok", "zero"}


def nonzero_divisor(a):
    try:
        r = a / 2
        how = "ok"
    except ZeroDivisionError:
        how = "zero"
    return how
    #@value how: includes={"ok"}
    #@todo value how: exactly={"ok"}  (a JSON value is any object to rowan: its `/` may run anything)


def added(a, b):
    try:
        r = a + b
        how = "ok"
    except TypeError:
        how = "type"
    return how
    #@value how: exactly={"ok", "type"}


def typed_add(a: int, b: int):
    try:
        r = a + b
        how = "ok"
    except TypeError:
        how = "type"
    return how
    #@general how: exactly={"ok"}


def literal_zero():
    try:
        r = 1 / 0
        how = "ok"
    except ZeroDivisionError:
        how = "zero"
    return how
    #@value how: exactly={"zero"}


def literal_fine():
    try:
        r = 7 // 2
        how = "ok"
    except ZeroDivisionError:
        how = "zero"
    return how
    #@value how: exactly={"ok"}


def shifted(a, n):
    try:
        r = a << n
        how = "ok"
    except ValueError:
        how = "negative"
    return how
    #@value how: exactly={"ok", "negative"}


def stage_in_finally(a, b):
    st = 1
    try:
        st = 2
        r = a % b
        st = 3
    finally:
        z = st
        #@value z: exactly={2, 3}
    return r


def accumulate(rows):
    total = 0
    try:
        for r in rows:
            total += r
        how = "ok"
    except TypeError:
        how = "type"
    return how
    #@value how: exactly={"ok", "type"}


ratio(DATA["x"], DATA["y"])
typed_ratio(6, 3)
nonzero_divisor(DATA["x"])
added(DATA["x"], DATA["s"])
typed_add(1, 2)
literal_zero()
literal_fine()
shifted(DATA["k"], DATA["y"])
stage_in_finally(DATA["x"], DATA["y"])
accumulate(DATA.values())

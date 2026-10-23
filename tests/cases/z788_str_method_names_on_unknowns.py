"""A method with a str method's name, called on an unknown not known to be a str, gives a value
of no known type: bytes have the same methods, dates and the program's objects a `replace`.
On one annotated `str` (or `bytes`) the result is of that type. (Found by the held-out corpus:
isodate's `Duration.__add__`, `newdt = other.replace(...)` on a date, typed `str(?)`.)"""
import datetime


def shift(other):
    newdt = other.replace(year=2001)  #@general newdt: untyped
    return newdt


def shout(x):
    up = x.upper()  #@general up: untyped
    return up


def shout_text(x: str):
    up = x.upper()  #@general up: types={str}
    return up


d = shift(datetime.date(2000, 1, 2))
b = shout(b"abc")  #@value b: includes={b'ABC'}
s = shout_text("abc")  #@value s: includes={'ABC'}

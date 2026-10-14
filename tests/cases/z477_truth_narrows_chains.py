"""Hypothesis: a truth test narrows the name or attribute chain it reads (on its true path
None, 0 and "" are not among its values), and a class attribute rebound after the class was
made reads as it is on this path. functional's hypothesis did
`settings._current_profile and settings._profiles[settings._current_profile]` and was told the
read fails at key None; a class attribute read gave the last value stored anywhere.

Expectations were written from the program text before running the engine."""
import sys


class Settings:
    profiles = {}
    current = None


def register(name):
    if Settings.current and Settings.profiles[Settings.current] == "x":
        return 1
    Settings.profiles[name] = name
    return 0


def load(name):
    Settings.current = name


first = register("a")
#@value first: exactly={0}
if len(sys.argv) > 5:
    load("a")
cur = Settings.current
#@value cur: exactly={None,"a"}
second = register("b")
#@value second: exactly={0}

x = None if len(sys.argv) > 5 else "id"
if x:
    y = x.upper()
    #@value y: exactly={"ID"}

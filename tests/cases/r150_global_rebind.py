"""Hypothesis: `global X; X = {...}` in a function rebinds the module name, so later reads
(through another function) see the new dict while an earlier reference keeps the old one.

Expectations were written from the program text before running the engine."""

STATE = {"phase": "init"}


def advance():
    global STATE
    STATE = {"phase": "run", "started": True}


def peek():
    return STATE


before = peek()                              #@row before: present={phase} definite={phase} closed
advance()
after = peek()                               #@row after: present={phase,started} definite={phase,started} closed
before["started"]                            #@ missing started
_ = 0                                        #@row STATE: present={phase,started} definite={phase,started} closed

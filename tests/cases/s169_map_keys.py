"""Hypothesis: `map` with an unbound method (`str.upper`) or a lambda over known items gives
known results usable as keys.

Expectations were written from the program text before running the engine."""

KEYS = ["a", "b"]
up = dict.fromkeys(map(str.upper, KEYS), 0)  #@row up: present={A,B} definite={A,B} closed
pairs = dict(map(lambda k: (k, len(k)), ["xx", "y"]))  #@row pairs: present={xx,y} definite={xx,y} closed
up["a"]                                      #@ missing a

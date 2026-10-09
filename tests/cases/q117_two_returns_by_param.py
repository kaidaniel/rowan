"""Hypothesis: a function that returns one of two different dicts depending on a literal
argument gives each caller only its own dict (no join across call sites).

Expectations were written from the program text before running the engine."""


def pick(kind):
    if kind == "full":
        return {"id": 1, "name": "n", "extra": True}
    return {"id": 1}


full = pick("full")                       #@row full: present={id,name,extra} definite={id,name,extra} closed
lite = pick("lite")                       #@row lite: present={id} definite={id} closed
lite["name"]                              #@ missing name
full["name"]                              #@ ok name

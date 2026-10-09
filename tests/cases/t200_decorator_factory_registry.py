"""Hypothesis: a decorator factory `@handles("kind")` registers each function under its kind
at definition time; calling through the registry reaches the right function.

Expectations were written from the program text before running the engine."""

HANDLERS = {}


def handles(kind):
    def deco(fn):
        HANDLERS[kind] = fn
        return fn
    return deco


@handles("create")
def on_create(evt):
    return {"created": evt["id"]}


@handles("delete")
def on_delete(evt):
    return {"deleted": evt["id"]}


_ = 0                                        #@row HANDLERS: present={create,delete} definite={create,delete} closed
r = HANDLERS["create"]({"id": 1})            #@row r: present={created} definite={created} closed
HANDLERS["update"]                           #@ missing update

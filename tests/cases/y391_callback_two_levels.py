"""A callback passed down two levels before it is called.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def run(handler, payload):
    return dispatch(handler, payload)


def dispatch(h, p):
    res = h(p)
    _ = 0                                    #@row res: present={handled,p} definite={handled,p} closed
    return res


def on_event(p):
    return {"handled": True, "p": p}


run(on_event, {"k": 1})

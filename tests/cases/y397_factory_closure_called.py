"""A closure from a factory, called once: inside it, the key captured from the factory.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def make_setter(key):
    def setter(d):
        d[key] = True
        _ = 0                                #@row d: present={a} definite={a} closed
        return d
    return setter


set_a = make_setter("a")
set_a({})

"""A callback mapped over rows inside a function: the mapped element is known.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def transform(rows, fn):
    out = [fn(r) for r in rows]
    first = out[0]
    _ = 0                                    #@row first: present={id,ok} definite={id,ok} closed
    return out


transform([{"id": 1}], lambda r: {**r, "ok": True})

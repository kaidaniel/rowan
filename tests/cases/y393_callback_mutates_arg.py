"""A callback that mutates the argument: the caller sees the key after the call.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def with_audit(fn, rec):
    fn(rec)
    _ = 0                                    #@row rec: present={id,audited} definite={id,audited} closed
    return rec


def mark(r):
    r["audited"] = True


with_audit(mark, {"id": 1})

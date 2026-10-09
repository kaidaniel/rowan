"""An uncalled function with `opts=None`: a caller may pass a dict (or nothing), so after
`opts = opts or {}` the dict is open, and `setdefault` makes its key definite.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def configure(opts=None):
    opts = opts or {}
    opts.setdefault("level", 1)
    _ = 0                                    #@row opts: present={level} definite={level} open
    lvl = opts["level"]                      #@ ok level
    return opts

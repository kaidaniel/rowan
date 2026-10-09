"""An uncalled function indexing open rows by a data key: the index is open; reads on the
elements are requirements on them.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def index(rows):
    by_id = {}
    for r in rows:
        by_id[r["id"]] = r                   #@ input id
    _ = 0                                    #@row by_id: present={} definite={} open
    return by_id
#@schema param index.rows['*']: always={id}

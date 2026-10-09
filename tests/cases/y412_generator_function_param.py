"""A generator function passed in and iterated inside: batch sizes of known items.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def batches(items, size):
    batch = []
    for it in items:
        batch.append(it)
        if len(batch) == size:
            yield batch
            batch = []
    if batch:
        yield batch


def consume(gen_fn, items):
    total = {}
    for b in gen_fn(items, 2):
        total[len(b)] = True
    _ = 0                                    #@row total: present={2,1} definite={2,1} closed
    return total


consume(batches, [1, 2, 3])

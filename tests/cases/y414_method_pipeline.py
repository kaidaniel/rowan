"""A method applying the functions another method collected on the instance.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


class Pipeline:
    def __init__(self):
        self.steps = []

    def add(self, fn):
        self.steps.append(fn)
        return self

    def run(self, d):
        for s in self.steps:
            d = s(d)
        _ = 0                                #@row d: present={a,b} definite={a,b} closed
        return d


p = Pipeline().add(lambda d: {**d, "a": 1}).add(lambda d: {**d, "b": 1})
p.run({})

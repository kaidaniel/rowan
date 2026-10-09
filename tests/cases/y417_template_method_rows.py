"""A base method calling an overridden hook, run for two classes: inside, the join.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


class Base:
    def build(self):
        d = self.start()
        d["base"] = True
        _ = 0                                #@row d: present={a,b,base} definite={base} closed
        return d

    def start(self):
        return {"a": 1}


class Sub(Base):
    def start(self):
        return {"b": 1}


Base().build()
Sub().build()

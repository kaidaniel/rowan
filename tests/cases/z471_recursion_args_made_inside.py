"""Hypothesis: a recursion whose deeper calls are given objects made inside it (a node built
in the recursion and asked for its name) restarts its fixpoint from those arguments with the
heap they were made in, so an attribute every `__init__` sets is there. The restart joined
the arguments but started from the heap before the call, where the new objects had no
attributes: `self.exprs` was reported never set. Reduced from pyparsing's ParserElement names
(`str(self.anchor + Empty())` inside `_generateDefaultName`).

Expectations were written from the program text before running the engine."""
import sys


class El:
    def __init__(self):
        self.cn = None

    def name(self):
        return self.cn if self.cn is not None else self.gen()   #@ ok cn ; sound cn

    def add(self, other):
        if len(sys.argv) > 5:
            return Skip(self)
        return And([self, other])


class Empty(El):
    def gen(self):
        return "Empty"


class And(El):
    def __init__(self, exprs):
        super().__init__()
        self.exprs = exprs

    def gen(self):
        return " ".join(e.name() for e in self.exprs)          #@ ok exprs ; sound exprs


class Skip(El):
    def __init__(self, anchor):
        super().__init__()
        self.anchor = anchor

    def gen(self):
        return self.anchor.add(Empty()).name()                 #@ ok anchor ; sound anchor


t = Skip(Skip(Empty())).name()

"""Hypothesis: an iterator class whose __iter__ returns self is iterated through __next__
(the analysis ended in a stack overflow asking __iter__ again and again); its items are the
records __next__ returns, the loop may take none."""


class Lines:
    def __init__(self, xs):
        self._lines = list(xs)

    def __iter__(self):
        return self

    def __next__(self):
        if not self._lines:
            raise StopIteration
        return {"line": self._lines.pop(0), "n": len(self._lines)}


out = []
for rec in Lines(["a", "b"]):
    out.append(rec["line"])                       #@ ok line
    #@row rec: present={line,n} definite={line,n} closed
first = out[0]                                    #@todo value first: exactly={"a"}
#@value first: includes={"a"}
n = len(out)                                      #@todo value n: exactly={2}
#@value n: includes={2}

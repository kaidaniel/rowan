"""Hypothesis: a method past its budget still stores, on every way it completes, the
attributes its own top-level statements assign before anything that may return: they are
there after the call (with unknown values). They were "only set on some paths" (rich's
`Text.__init__`, then every `len(text)`); what it assigns after a possible return, or only
in a branch, stays possible.

Expectations were written from the program text before running the engine."""
#@limits budget=0
import sys


class Text:
    def __init__(self, text, style=""):
        self._text = [text]
        self.style = style
        self._length = len(text)
        if style:
            self.styled = True

    def grow(self, more):
        if not more:
            return self
        self._length += len(more)
        self.last = more
        return self


warm = Text("x")                               # (the first call runs: budget 0 skips the next)
t = Text("abc", sys.argv[0])
n = t._length                                  #@ ok _length
s = t.style                                    #@ ok style
maybe = t.styled                               #@ possibly styled
warm.grow("")
t.grow(sys.argv[0])
grown = t._length                              #@ ok _length
last = t.last                                  #@ possibly last

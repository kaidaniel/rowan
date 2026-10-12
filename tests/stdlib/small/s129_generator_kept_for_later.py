"""A generator's body runs when it is consumed, not when it is made. rowan runs it where it is
made: right for `for x in g()`, wrong for one kept for later. Here `__init__` makes the
generator before setting `self.cur`, and the body only runs at `p.step()`, when `cur` is there
(email's FeedParser: `self._parse = self._parsegen().__next__`). It read `self.cur` as
definitely missing."""


class Parser:
    def __init__(self):
        self.step = self._run().__next__
        self.cur = None

    def _run(self):
        while True:
            if self.cur is None:                  #@ ok cur
                self.cur = "start"
            yield self.cur


p = Parser()
first = p.step()                                  #@value first: exactly={"start"}

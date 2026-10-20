"""`hasattr(cls, name)` on a class with a base rowan does not see (here `HTMLParser`, analysed
as a library rowan does not run) decides nothing: the base may have the name. Before, the test
narrowed the class's own attributes, where `feed` is not, so `check` was taken to always raise.

Expectations were written from the program text before running the engine."""
from html.parser import HTMLParser


class Reader(HTMLParser):
    def check(self, name):
        if not hasattr(type(self), name):
            raise TypeError(name)
        return name


r = Reader().check("feed")                   #@value r: exactly={"feed"}
done = True                                  #@value done: exactly={True}

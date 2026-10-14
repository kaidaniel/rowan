# A loop runs to its fixpoint by joining each round's back edge into its head: the rounds of
# `rich.protocol.rich_cast` (a `while` over what `__rich__` returns, with a `break` on a type
# seen before) used to alternate between two heads until the 40-round cut-off.
from inspect import isclass


class Text:
    def __init__(self, s):
        self.s = s

    def __repr__(self):
        return f"<text {self.s!r}>"


class Panel:
    def __init__(self, r):
        self.r = r

    def __rich__(self):
        return Text("p")


def rich_cast(renderable):
    seen = set()
    while hasattr(renderable, "__rich__") and not isclass(renderable):
        if hasattr(renderable, "zzz"):
            return repr(renderable)
        cast_method = getattr(renderable, "__rich__")
        renderable = cast_method()
        t = type(renderable)
        if t in seen:
            break
        seen.add(t)
    return renderable


def render(x):
    return rich_cast(x)


a = render(Text("a"))
sa = a.s                                         #@ ok s
#@value sa: exactly={"a"}
b = render(Panel(Text("b")))
c = render("s")
# (a str has no `__rich__`: `dir(str)` is known)
#@value c: exactly={"s"}
#@work <= 400

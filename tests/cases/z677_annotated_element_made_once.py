# The instance standing for every element of a parameter annotated `dict[str, list[B]]` is made
# once per path, its `__init__` run on it: read again in a loop, it is that one, so the stores of
# `__init__` hold for it (`self.name` set by every path that completes). Nothing calls f: its run
# is for any call.
class Parsed:
    def __init__(self, name):
        self.name = name


def parse(s):
    if not s:
        raise ValueError("empty")
    return Parsed(s.strip())


class A:
    def __init__(self, s: str) -> None:
        try:
            parsed = parse(s)
        except ValueError as e:
            raise RuntimeError(str(e)) from e
        self.name: str = parsed.name


class B(A):
    def __init__(self, s: str):
        super().__init__(s)
        self.unsafe = self.name
        #@ ok name


def f(dm: dict[str, list[B]]):
    out = []
    for k in dm:
        for r in dm[k]:
            out.append(r.name)
            #@ ok name
    for k in dm:
        for r in dm[k]:
            out.append(r.unsafe)
            #@ ok unsafe
    return out

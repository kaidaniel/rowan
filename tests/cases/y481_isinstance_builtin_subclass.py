"""An instance of a user class deriving from a builtin class is an instance of that builtin:
`isinstance(C(), dict)` for `class C(dict)`; and its truth is the builtin's (an empty
`class C(list)` instance is falsy). Both were decided (False / truthy): the branch was dead."""


class Config(dict):
    pass


class Items(list):
    pass


c = Config(a=1)
kind = "mapping" if isinstance(c, dict) else "other"
#@value kind: includes={"mapping"}
i = Items([1])
seq = "seq" if isinstance(i, (list, tuple)) else "other"
#@value seq: includes={"seq"}


class Bag(list):
    pass


empty = Bag()
t = "full" if empty else "empty"
#@value t: includes={"empty"}

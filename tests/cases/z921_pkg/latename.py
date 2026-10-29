"""For z921_reimport.py: a decorator at the `def` reads a name the module binds only later
(NameError, every time the module's code runs)."""


def tag(fn):
    fn.label = LATER
    return fn


@tag
def f():
    return 1


LATER = 1

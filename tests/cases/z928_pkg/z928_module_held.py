"""A module imported inside a function stays in `sys.modules` after the function returns: its
globals are there for any later call that imports it again, also after the call stores another
attribute on it. Rowan's collector dropped what it knew of the module when nothing in the
function's caller held it; a store on it then made it a module with that attribute only, and
`m.LABEL` was claimed to raise AttributeError for any call.

Expectations were written from the program text before running the engine."""


def read_label():
    import settingslib as m

    m.OTHER = 1
    try:
        r = m.LABEL
    except AttributeError:
        r = "AE"
    return r
    #@general r: includes={"first"}


v = read_label()                             #@value v: exactly={"first"}

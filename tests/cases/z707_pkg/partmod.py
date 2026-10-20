"""A decorator reading a name its module binds only after the decorated `def`
(z707_deferred_hazards.py): at the `def` the name is unbound, so the decorator raises there."""


def note(fn):
    fn.note = LATE
    return fn


try:
    @note
    def early():
        return 1
except NameError:
    early = None

LATE = 1

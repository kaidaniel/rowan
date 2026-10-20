"""Half of a circular import (z707_deferred_hazards.py): `cycb` is imported before `deco` is
defined, and decorates with `cyca.deco` while this module is still loading."""
import cycb


def deco(fn):
    fn.mark = MARK
    return fn


MARK = "a"

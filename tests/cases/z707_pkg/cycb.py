"""The other half of the circular import with `cyca` (z707_deferred_hazards.py)."""
import cyca

try:
    @cyca.deco
    def g():
        return 1
except AttributeError:
    g = None

"""An object a library call rowan does not model made (`contextvars.ContextVar(...)`) has
methods of its own. Its `.get()` was taken for a dict's, which raises nothing, so the
`except LookupError:` around it never ran: `current()` was `?` alone, though an unset
ContextVar's `get()` raises LookupError and CPython gives "fresh". An input's data keeps
the dict and str methods (`row.get("id")`)."""
import contextvars
import json
import os

_v = contextvars.ContextVar("ctx")


def current():
    try:
        return _v.get()
    except LookupError:
        return "fresh"


c = current()
#@value c: shows={'fresh'} unknown

row = json.loads(os.environ.get("ROWAN_ROW", '{"id": 7}'))
i = row.get("id", 0)
#@value i: includes={0}

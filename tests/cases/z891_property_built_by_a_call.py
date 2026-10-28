"""`name = property(fget, fset, fdel)` in a class body makes the same data descriptor as the
`@property` / `@name.setter` decorators: reading `obj.name` runs `fget(obj)`, storing runs
`fset(obj, value)`, `del` runs `fdel(obj)`; a lambda may be the getter. Before, the call was
an unknown library call: `obj.name` read the property object itself, and the getter's effects
(an attribute it caches on the instance) never happened.

Expectations were written from the program text before running the engine."""


class Request:
    _cache = None

    def __init__(self, env):
        self.env = env

    def _headers__get(self):
        if self._cache is None:
            self._cache = {"host": self.env["host"]}
        return self._cache

    def _headers__set(self, value):
        self._cache = dict(value)

    def _headers__del(self):
        self._cache = None

    headers = property(_headers__get, _headers__set, _headers__del, doc="the headers")

    host = property(lambda self: self.env["host"])


req = Request({"host": "example.org"})
#@row req: present={env} definite={env} closed
h = req.headers
#@row req: present={env, _cache} definite={env, _cache} closed
host_header = h["host"]
#@value host_header: exactly={"example.org"}
again = req.headers is h
#@value again: exactly={True}

req.headers = [("host", "other.org")]
after = req.headers["host"]
#@value after: exactly={"other.org"}

del req.headers
cleared = req._cache
#@value cleared: exactly={None}

host = req.host
#@value host: exactly={"example.org"}

try:
    req.host = "x"
    stored = "stored"
except AttributeError:
    stored = "read-only"
seen = stored
#@value seen: exactly={"read-only"}

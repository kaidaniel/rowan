# A subclass of a functional NamedTuple with its own `__new__` that calls the base's through
# `super().__new__(cls, ...)` (urllib3's Url): the instance has the fields.
import typing


class Url(
    typing.NamedTuple(
        "Url",
        [
            ("scheme", typing.Optional[str]),
            ("host", typing.Optional[str]),
        ],
    )
):
    def __new__(cls, scheme=None, host=None):
        if scheme is not None:
            scheme = scheme.lower()
        return super().__new__(cls, scheme, host)

    @property
    def netloc(self):
        return self.host  #@ ok host


u = Url("HTTP", "example.org")
s = u.scheme  #@ ok scheme
#@value s: exactly={"http"}
h = u.netloc
#@value h: exactly={"example.org"}
v = Url(host="x")
n = v.scheme  #@ ok scheme
#@value n: exactly={None}

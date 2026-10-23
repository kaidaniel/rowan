"""A property keeps being one through `@x.deleter` and `@x.getter`: reading it runs the getter
(the last `@x.getter` one), not a bound method. (Found by the held-out corpus: webob's
`Request.body`, a property with a setter and a deleter, read as `body = self.body`.)"""


class Request:
    def __init__(self):
        self._body = b"raw"

    @property
    def body(self):
        return self._body

    @body.setter
    def body(self, value):
        self._body = value

    @body.deleter
    def body(self):
        self._body = b""


class Tagged:
    @property
    def tag(self):
        return "old"

    @tag.getter
    def tag(self):
        return "new"


r = Request()
b = r.body  #@value b: exactly={b'raw'}
r.body = b"set"
b2 = r.body  #@value b2: exactly={b'set'}
del r.body
b3 = r.body  #@value b3: exactly={b''}
t = Tagged().tag  #@value t: exactly={'new'}

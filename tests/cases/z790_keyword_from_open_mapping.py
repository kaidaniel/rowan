"""A parameter with a default, bound from `**kw` whose keys are not all known (a function's own
`**kw` for any call, data from an input), is what the mapping may hold there or the default.
(Found by the held-out corpus: voluptuous's validators, whose subclasses forward `**kwargs`
to `super().__init__(..., msg=None, **kwargs)`; `self.msg` was taken for None alone.)"""
import json
import os


def label(*items, msg=None, **rest):
    return msg


def plain(msg=None):
    return msg


def forward(**kw):
    r = label(**kw)  #@general r: shows={None} untyped
    r2 = plain(**kw)  #@general r2: shows={None} untyped
    return r


class Base:
    def __init__(self, *validators, msg=None, **kwargs):
        self.msg = msg


class Some(Base):
    def __init__(self, validators, **kwargs):
        super().__init__(*validators, **kwargs)

    def text(self):
        m = self.msg  #@general m: shows={None} untyped
        return m


opts = json.loads(os.environ.get("Z790_OPTS", '{"msg": "custom"}'))
a = forward(msg="given")  #@value a: includes={'given'}
b = label(**opts)  #@value b: includes={'custom'}
c = Some([], msg="max failed").text()  #@value c: includes={'max failed'}

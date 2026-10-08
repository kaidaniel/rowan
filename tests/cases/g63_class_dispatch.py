#@engine rust
"""Polymorphism: a dict of classes, methods overridden in subclasses, abstract bases."""


class Handler:
    def handle(self, p):
        raise NotImplementedError


class Add(Handler):
    def handle(self, p):
        return {"result": p["a"] + p["b"], "op": "add"}     #@ ok a ; ok b


class Neg(Handler):
    def handle(self, p):
        return {"result": -p["a"], "op": "neg", "unary": True}  #@ ok a


HANDLERS = {"add": Add, "neg": Neg}


def run(op, payload):
    cls = HANDLERS[op]
    return cls().handle(payload)


r = run("add", {"a": 1, "b": 2})
r["result"]                              #@ ok result
r["unary"]                               #@ missing unary
r2 = run(op_name, {"a": 1, "b": 2})
r2["op"]                                 #@ ok op
r2["unary"]                              #@ possibly unary
r2["zzz"]                                #@ missing zzz
instances = [Add(), Neg()]
for h in instances:
    out = h.handle({"a": 1, "b": 2})
    out["op"]                            #@ ok op
    out["unary"]                         #@ possibly unary


class Base:
    def describe(self):
        return {"kind": self.kind(), **self.extra()}

    def kind(self):
        return "base"

    def extra(self):
        return {}


class Special(Base):
    def kind(self):
        return "special"

    def extra(self):
        return {"level": 1}


Special().describe()["level"]            #@ ok level
Base().describe()["level"]               #@ missing level
Special().describe()["kind"]             #@ ok kind

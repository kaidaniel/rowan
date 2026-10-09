"""A store through a list index that is not known (`self.objects[oid].payload = p` with
`oid` computed): it may have hit any element, so the store is weak on every element and
the earlier payload keys become possible, not definite.

Expectations were written from the program text before running the engine."""
class Obj:
    def __init__(self, payload):
        self.payload = payload


class Table:
    def __init__(self):
        self.objects = []

    def alloc(self, payload):
        o = Obj(payload)
        self.objects.append(o)
        return len(self.objects) - 1

    def tag(self, oid, p):
        self.objects[oid].payload = p


t = Table()
i = t.alloc({"kind": "f", "qualname": "x"})
j = t.alloc({"kind": "d"})
k = int("1")
t.tag(k, {"kind": "ph"})
# The index is unknown, so the store may have hit either object: qualname is possible.
first = t.objects[0].payload              #@row first: present={kind,qualname} definite={kind} closed
first["qualname"]                         #@ possibly qualname

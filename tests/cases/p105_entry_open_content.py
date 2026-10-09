"""An entry point (a method nothing calls, analysed with an open `self`): a helper stores
a payload on `self.objects[oid]` for an unknown `oid`, and the method later reads
`self.objects[f].payload.qualname` for another unknown index.  The element under an
unknown key stands for many objects, so the store is weak and the read sees the stored
payload (which has no qualname) or the unknown original: possibly missing, not definitely.
This was a false "definitely missing" on the old prototype used as a fixture.

Expectations were written from the program text before running the engine."""
from typing import Any, Optional


class PH:
    def __init__(self, kind, func, name="", parent=None, ns="item", key=None, cls=None):
        self.kind = kind
        self.func = func
        self.name = name
        self.parent = parent
        self.ns = ns
        self.key = key
        self.cls = cls


class SummaryMixin:
    def new_placeholder(self, kind: str, func: int, name: str = "", parent: Optional[int] = None,
                        ns: str = "item", key: Any = None, cls: Optional[int] = None, tag: str = "") -> int:
        if kind == "path":
            site = f"ph:path:{func}:{parent}:{ns}:{key!r}{tag}"
        else:
            site = f"ph:{kind}:{func}:{name}"
        oid = self._alloc("ph", site, f"sum{func}", True, f"ρ{name or key!r}", payload=PH(kind, func, name, parent, ns, key, cls))
        self.objects[oid].payload = PH(kind, func, name, parent, ns, key, cls)
        if kind == "path":
            self.objects[oid].depth = (self.objects[parent].depth + 1) if parent is not None else 1
            self.objects[oid].iter_elem = key == "*"
        return oid

    def record_event(self, kind, ph):
        if self.sum_stack and (self.objects[ph].kind == "ph" or self.is_entry_object(ph)):
            self.sum_events[-1][(kind, ph)] = kind

    def summarise(self, foid: int):
        self.sum_stack.append(foid)
        self.sum_events.append({})
        try:
            for rnd in range(3):
                sp = self.new_placeholder("self", foid, "self")
                pp = self.new_placeholder("path", foid, parent=sp, key="x")
                self.record_event("narrow", pp)
                if rnd >= 2:
                    names = [self.objects[f].payload.qualname for f in self.sum_stack]  #@ possibly qualname
        finally:
            self.sum_stack.pop()

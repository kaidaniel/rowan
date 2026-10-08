"""Operations over stores and items; imports both other modules in different styles."""
import models as m
import store
from store import open_store


class Special(m.Item):
    def label(self):
        d = m.Item.label(self)
        d["special"] = True
        return d


def seed(name):
    s = open_store(name)
    s.add(m.Item("a", 1.0).with_tag("new"))
    s.add(m.Bundle("b", [m.Item("x", 2.0), m.Item("y", 3.0)]))
    s.add(Special("c", 4.0))
    return s


def report(name):
    s = store.STORES[name]
    out = {"store": s.summary(), "labels": {}}
    for sku, item in s.items.items():
        out["labels"][sku] = item.label()
    out["first_log"] = s.log[0]
    return out

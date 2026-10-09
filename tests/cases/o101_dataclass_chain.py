"""A three-level dataclass chain with defaults, `__post_init__` at two levels, a classmethod
constructor, a generator with `yield from`, and nested context managers that record into
one dict.

Expectations were written from the program text before running the engine."""
from dataclasses import dataclass, field


@dataclass
class Node:
    name: str
    attrs: dict = field(default_factory=dict)

    def __post_init__(self):
        self.key = "n:" + self.name


@dataclass
class Branch(Node):
    children: list = field(default_factory=list)

    def __post_init__(self):
        super().__post_init__()
        self.count = len(self.children)

    @classmethod
    def of(cls, name, *children):
        return cls(name, {}, list(children))


@dataclass
class Root(Branch):
    region: str = "eu"


def leaves(node):
    if isinstance(node, Branch):
        for c in node.children:
            yield from leaves(c)
    else:
        yield node


class Span:
    def __init__(self, store, name):
        self.store = store
        self.name = name

    def __enter__(self):
        self.store[self.name] = {"open": True}
        return self

    def __exit__(self, *exc):
        self.store[self.name]["closed"] = True


root = Root.of("r", Branch.of("b1", Node("x"), Node("y")), Node("z"))
_ = 0                                     #@row root: present={name,attrs,children,region,key,count} definite={name,attrs,children,region,key,count} closed
b1 = root.children[0]                     #@row b1: present={name,attrs,children,key,count} definite={name,attrs,children,key,count} closed
z = root.children[1]                      #@row z: present={name,attrs,key} definite={name,attrs,key} closed
found = list(leaves(root))
spans = {}
with Span(spans, "outer"):
    with Span(spans, "inner"):
        pass
_ = 0                                     #@row spans: present={outer,inner} definite={outer,inner} closed
outer = spans["outer"]                    #@row outer: present={open,closed} definite={open,closed} closed
z.children                                #@ missing children
root.region                               #@ ok region

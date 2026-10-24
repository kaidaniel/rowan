"""getattr by a name made with `+` from a literal prefix (`"visit_" + kind`, kind read from
data): only the attributes whose names start with it may be read, so `reset` (which would set
`calls` to None) and `__init__` are not called through it. (`n`: one visit appends once at most.)"""
import json
import sys


class Visitor:
    def __init__(self):
        self.calls = []

    def visit(self, node):
        method = getattr(self, "visit_" + node["type"], self.generic_visit)
        return method(node)

    def generic_visit(self, node):
        return "generic"

    def visit_Name(self, node):
        self.calls.append("name")
        return "name"

    def visit_Num(self, node):
        return "num"

    def reset(self, node):
        self.calls = None
        return "reset"


v = Visitor()
node = json.loads(sys.argv[1] if len(sys.argv) > 1 else '{"type": "Num"}')
r = v.visit(node)
#@value r: exactly={'name', 'num', 'generic'}
n = len(v.calls)
#@value n: exactly={0, 1}

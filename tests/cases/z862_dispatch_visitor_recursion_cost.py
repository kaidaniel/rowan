"""A visitor that dispatches through `getattr(self, "visit_" + type)` and whose handlers call
`visit` again on the children: mutual recursion through a call of several callees, the shape
of a linter's node handlers or an HTML parser's phases. The root needs "type" on every path;
the result is one of the handlers' (or the default's). What it costs is the nesting of
activations per call site and level, not rounds (`#@work`: today's cost, so that it does not
grow; iteration 93 measured the redesigns that would shrink it, PR #96)."""
import json
import sys


class V:
    def __init__(self):
        self.names = []

    def visit(self, node):
        handler = getattr(self, "visit_" + node["type"], self.generic)
        return handler(node)

    def generic(self, node):
        for child in node.get("body", []):
            self.visit(child)
        return "generic"

    def visit_Name(self, node):
        self.names.append(node["id"])
        return "name"

    def visit_Call(self, node):
        self.visit(node["func"])
        for a in node["args"]:
            self.visit(a)
        return "call"


r = V().visit(json.loads(sys.stdin.read()))
#@value r: exactly={'generic', 'name', 'call'}
#@schema json.loads@L35: always={type}
#@work <= 13000

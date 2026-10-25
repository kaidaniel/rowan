"""Mutual recursion over an input tree: a recursive activation's rounds stop once its result and
the heap it leaves are stable, whatever its own local names (`a`, the loop's item) hold at its
end: the caller takes nothing else from it."""
import json
import sys


class V:
    def visit(self, node):
        if node["t"] == "C":
            return self.visit_C(node)
        return None

    def visit_C(self, node):
        self.visit(node["f"])
        for a in node["args"]:
            self.visit(a)
        return "c"


r = V().visit(json.loads(sys.stdin.read()))
#@value r: exactly={'c', None}
#@schema json.loads@L21: always={t}
#@work <= 2200

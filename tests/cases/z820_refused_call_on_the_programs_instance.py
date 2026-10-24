"""A method whose budget refuses a call gets a reporting run for that call with open parameters,
on an instance made anew with `__init__`'s defaults, not the one the call had: here the second
call finds what the first stored, which that run cannot see (its claim at `n` is None alone).
Taking the program's own instances there instead cost the eight benchmarks many ⚠ (rich 2 ->
50): a known gap (the PR's Remaining)."""
#@limits budget=0


class G:
    def __init__(self):
        self.nodes = {}

    def node(self, name):
        n = self.nodes.get(name)
        #@value n: includes={None}
        #@todo value n: includes={None} untyped  (precisely: None or the list the first call stored)
        if n is None:
            n = [name]
            self.nodes[name] = n
        return n


g = G()
a = g.node("a")
b = g.node("a")

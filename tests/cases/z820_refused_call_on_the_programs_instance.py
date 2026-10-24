"""A method whose budget refuses a call runs for that call with open parameters, on any of the
program's instances (an instance made anew with `__init__`'s defaults is not the one the call
had): here the second call finds what the first stored."""
#@limits budget=0


class G:
    def __init__(self):
        self.nodes = {}

    def node(self, name):
        n = self.nodes.get(name)
        #@value n: includes={None} untyped
        # (precisely: None or the list the first call stored)
        if n is None:
            n = [name]
            self.nodes[name] = n
        return n


g = G()
a = g.node("a")
b = g.node("a")

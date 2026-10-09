"""Hypothesis: an attribute set on each instance of a known list in a loop is set on every
one, with its own value.

Expectations were written from the program text before running the engine."""


class Node:
    def __init__(self, name):
        self.name = name


nodes = [Node(n) for n in ["a", "b"]]
for nd in nodes:
    nd.meta = {"for": nd.name}
m0 = nodes[0].meta                           #@row m0: present={for} definite={for} closed
k = {nodes[1].meta["for"]: 1}                #@row k: present={b} definite={b} closed

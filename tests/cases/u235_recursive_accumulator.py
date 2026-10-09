"""Hypothesis: a recursive walk of a known tree that stores into an accumulator dict passed
down records every node.

Expectations were written from the program text before running the engine."""


def collect(node, acc):
    acc[node["name"]] = True
    for ch in node.get("children", []):
        collect(ch, acc)
    return acc


TREE = {"name": "root", "children": [{"name": "a"}, {"name": "b", "children": [{"name": "c"}]}]}
names = collect(TREE, {})                    #@row names: present={root,a,b,c} definite={root,a,b,c} closed

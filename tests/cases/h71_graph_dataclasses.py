"""Dataclass nodes and edges linked into a graph (nodes hold edges, edges hold nodes), built
through a Graph object with **kwargs, traversed with a stack, grouped with a defaultdict.

Expectations were written from the program text before running the engine."""
from collections import defaultdict
from dataclasses import asdict, dataclass, field


@dataclass
class Node:
    name: str
    attrs: dict = field(default_factory=dict)
    out: list = field(default_factory=list)


@dataclass
class Edge:
    src: Node
    dst: Node
    weight: int = 1
    meta: dict = field(default_factory=dict)


class Graph:
    def __init__(self):
        self.nodes = {}

    def node(self, name, **attrs):
        n = self.nodes.get(name)
        if n is None:
            n = Node(name, dict(attrs))
            self.nodes[name] = n
        return n

    def link(self, a, b, **meta):
        e = Edge(self.node(a), self.node(b), meta=meta)
        e.src.out.append(e)
        return e


g = Graph()
start = g.node("s", color="red")          #@row start: present={name,attrs,out} definite={name,attrs,out} closed
attrs = start.attrs                       #@row attrs: present={color} definite={color} closed
e1 = g.link("s", "a", kind="road")        #@row e1: present={src,dst,weight,meta} definite={src,dst,weight,meta} closed
m1 = e1.meta                              #@row m1: present={kind} definite={kind} closed
e2 = g.link("a", "b")
m2 = e2.meta                              #@row m2: present={} definite={} closed
e3 = g.link("s", "b", kind="rail", toll=True)
m3 = e3.meta                              #@row m3: present={kind,toll} definite={kind,toll} closed
nodes = g.nodes                           #@row nodes: present={s,a,b} definite={s,a,b} closed
a_attrs = g.nodes["a"].attrs              #@row a_attrs: present={} definite={} closed
via = e1.src.out[0].dst                   #@row via: present={name,attrs,out} definite={name,attrs,out} closed
rec = asdict(e1)                          #@row rec: present={src,dst,weight,meta} definite={src,dst,weight,meta} closed


def neighbours(node):
    return [(e.dst, e) for e in node.out]


def walk(g, name):
    visited = {}
    stack = [g.nodes[name]]
    while stack:
        n = stack.pop()
        if n.name in visited:
            continue
        visited[n.name] = {"node": n, "edges": len(n.out)}
        for dst, e in neighbours(n):
            if e.meta.get("toll"):
                visited[n.name]["toll_out"] = True
            stack.append(dst)
    return visited


# Corrected on re-reading (r140 unrolls `while` loops whose test is decided at every head): the stack holds known
# nodes, so the walk visits s, a, b; s's edge e3 has toll=True.
seen = walk(g, "s")                       #@row seen: present={s,a,b} definite={s,a,b} closed
entry = seen["s"]                         #@row entry: present={node,edges,toll_out} definite={node,edges,toll_out} closed
entry["toll_out"]                         #@ ok toll_out
entry["node"]                             #@ ok node


def group_by_color(g):
    groups = defaultdict(list)
    for n in g.nodes.values():
        groups[n.attrs.get("color", "none")].append(n.name)
    return dict(groups)


# The loop over the three known nodes is unrolled: "red" (s) and "none" (a, b) are both
# definitely inserted.
cols = group_by_color(g)                  #@row cols: present={red,none} definite={red,none} closed


def summarize(g):
    out = {"n_nodes": len(g.nodes), "n_edges": 0}
    for n in g.nodes.values():
        out["n_edges"] += len(n.out)
    if any(e.meta.get("toll") for n in g.nodes.values() for e in n.out):
        out["has_toll"] = True
    return {**out, "name": "g"}


# Corrected on re-reading (r157 added `any`/`all` models): e3 = link("s", "b", toll=True) is in
# s.out, so the any() over every known edge is True and has_toll is definite.
summ = summarize(g)                       #@row summ: present={n_nodes,n_edges,has_toll,name} definite={n_nodes,n_edges,has_toll,name} closed
summ["n_edges"]                           #@ ok n_edges
summ["has_toll"]                          #@ ok has_toll

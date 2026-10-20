"""Scenario 9: graph analysis on networkx (pure Python, a large package whose functions are
wrapped by dispatching decorators at import)."""

import networkx as nx

EDGES = [("a", "b", 4), ("b", "c", 1), ("a", "c", 7), ("c", "d", 2), ("e", "f", 1)]


def build(edges: list[tuple]) -> nx.Graph:
    g = nx.Graph()
    for u, v, w in edges:
        g.add_edge(u, v, weight=w)
    return g


def summary(g: nx.Graph) -> dict:
    path = nx.shortest_path(g, "a", "d", weight="weight")
    parts = [sorted(c) for c in nx.connected_components(g)]
    degrees = dict(g.degree())
    return {"path": path, "components": len(parts), "degrees": degrees, "nodes": g.number_of_nodes()}


if __name__ == "__main__":
    s = summary(build(EDGES))
    print(s["path"], s["components"], s["degrees"]["c"], s["nodes"])

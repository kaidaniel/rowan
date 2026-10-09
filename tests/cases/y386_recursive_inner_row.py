"""A recursive walk with an accumulator: inside, the accumulator over all activations.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def walk(node, acc):
    acc[node["name"]] = True
    _ = 0                                    #@row acc: present={root,a,b} definite={root} closed
    for ch in node.get("kids", []):
        walk(ch, acc)
    return acc


walk({"name": "root", "kids": [{"name": "a"}, {"name": "b"}]}, {})

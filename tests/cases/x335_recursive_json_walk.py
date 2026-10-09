"""Weakness: recursion over an input tree of unknown depth: the reads are requirements on the
root and on its children's path.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""
import json


def walk(node, out):
    out[node["id"]] = node
    for ch in node.get("children", []):
        walk(ch, out)
    return out


tree = json.loads(text_in)
seen = walk(tree, {})
#@schema json.loads@L17: always={id}
#@schema json.loads@L17['children']['*']: always={id}

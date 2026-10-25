"""A recursive walk whose body raises four classes, its recursive calls inside tries: each
recursive call raises what the walk raises, as one raise of any class, joined from the four
once per round of the recursion (not again at every recursive call)."""
import json
import sys


def check(node, seen):
    if not isinstance(node, dict):
        if node == "a":
            seen.key = node
            raise KeyError(node)
        if node == "b":
            seen.value = node
            raise ValueError(node)
        if node == "c":
            seen.type = node
            raise TypeError(node)
        if node == "d":
            seen.index = node
            raise IndexError(node)
        return 1
    total = 0
    for k in node:
        try:
            total += check(node[k], seen)
        except LookupError:
            caught = "lookup"
            #@value caught: exactly={'lookup'}
            total -= 1
        try:
            total += check(node[k], seen)
            total += check(node[k], seen)
            total += check(node[k], seen)
        finally:
            total += 1
    return total


class Seen:
    pass


check(json.loads(sys.argv[1]), Seen())
#@joins <= 500
#@work <= 3000

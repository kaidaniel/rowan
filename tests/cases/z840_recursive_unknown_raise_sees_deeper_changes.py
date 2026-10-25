"""A recursive call that may raise anything (unknown code in it, `json.loads`) after it changed
a list its caller made and passed down: the caller's handler sees the list as the deeper call
left it."""
import json
import sys


def walk(texts, seen):
    if not texts:
        return 0
    seen.append(texts[0])
    try:
        walk(texts[1:], seen)
    except ValueError:
        caught = len(seen)
        #@value caught: includes={2}
        return -1
    json.loads(texts[0])
    return len(seen)


def main():
    seen = []
    return walk(sys.argv[1:], seen)


main()

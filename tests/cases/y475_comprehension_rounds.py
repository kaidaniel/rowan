"""A comprehension over items of unknown number runs its body until what one iteration
leaves is what the next one starts from, as a `for` loop does. It ran the body once: a
walrus accumulator was `0 | 1`, and a second `pop` of the same key was proven to work."""
import os

xs = os.environ.get("XS", "abc").split(",")
t = 0
seen = [(t := t + 1) for x in xs]
#@value t: includes={0, 1, 2}

d = {"a": 1}
popped = [d.pop("a") for x in xs]                #@ possibly a

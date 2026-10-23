"""A loop that slices what it sliced (`pairs = pairs[2:]`) of a value holding several tuples:
one slice object per tuple sliced, up to a few, so the objects stop growing and the loop
reaches its fixpoint in a few rounds (it made new objects every round, never converging)."""
import os


def replace(text, *pairs):
    while pairs:
        text = pairs[1].join(text.split(pairs[0]))
        pairs = pairs[2:]
    return text


out = replace(os.environ.get("Z799", "abc"), "a", "b", "c", "d")
#@objects <= 100
#@work <= 100

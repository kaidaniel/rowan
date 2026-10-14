"""Hypothesis: a subscript's index or slice bounds, and a comparison's operands, are evaluated
once. A slice of something that is not a list of known length evaluated its bounds up to
three times (for a list slice, for a string slice, for the key), and a membership test not
decided evaluated its container again: a call there ran, and counted, more than once.

Expectations were written from the program text before running the engine."""
import sys

n = 0


def bump():
    global n
    n += 1
    return n


data = {"a": 1} if len(sys.argv) > 5 else sys.argv
part = data[:bump()]
calls = n
#@value calls: exactly={1}
text = "abc" if len(sys.argv) > 5 else "xyz"
first = text[bump() - 2]
after = n
#@value after: exactly={2}
#@value first: exactly={"a","x"}


def nums():
    global n
    n += 1
    return [1, 2] if len(sys.argv) > 5 else [3]


found = 3 in nums()
after_in = n
#@value after_in: exactly={3}

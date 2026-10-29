# Hypothesis: a tensor op that fails raises what torch raises, so a try's handler for that class
# runs: a dim out of range or an index past a dim IndexError, a size mismatch RuntimeError. A
# matmul of one of two tensors may fail on one of them only; an index into a dim of size 0
# fails; `chunk` of a dim of size 0 gives that many empty pieces. Before, every failure was a
# RuntimeError, an index was never checked, and a mismatch rowan could not decide raised nothing.
# Expectations were written from the program text before running the engine.
import sys

import torch


def transposed(x):
    try:
        x.transpose(0, 1)               #@shape-error
        how = "ok"
    except IndexError:
        how = "index"
    except RuntimeError:
        how = "runtime"
    return how


def matmul(a, b):
    try:
        a @ b                           #@shape-error
        how = "ok"
    except RuntimeError:
        how = "runtime"
    return how


def first(x):
    try:
        x[0]                            #@shape-error
        how = "ok"
    except IndexError:
        how = "index"
    return how


def pieces(x):
    return len(x.chunk(3, 0))


if __name__ == "__main__":
    t1 = transposed(torch.ones(4))
    #@value t1: exactly={"index"}
    m1 = matmul(torch.zeros(2, 3), torch.zeros(2, 3))
    #@value m1: exactly={"runtime"}
    ab = torch.zeros(2, 3) if len(sys.argv) > 5 else torch.zeros(3, 3)
    m2 = matmul(ab, ab)
    #@value m2: exactly={"ok", "runtime"}
    f1 = first(torch.zeros(0, 3))
    #@value f1: exactly={"index"}
    f2 = first(torch.zeros(2, 3))
    #@value f2: exactly={"ok"}
    n = pieces(torch.zeros(0, 3))
    #@value n: exactly={3}

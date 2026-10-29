# A numpy op that fails raises what numpy raises: a size mismatch ValueError (reshape, matmul),
# an axis out of range AxisError — both an IndexError and a ValueError, so either handler runs.
# Before, every failure was a RuntimeError, which neither handler catches.
# Expectations were written from the program text before running the engine.
import numpy as np


def reshaped(a):
    try:
        a.reshape(4, 4)                 #@shape-error
        how = "ok"
    except ValueError:
        how = "value"
    return how


def product(a):
    try:
        a @ a                           #@shape-error
        how = "ok"
    except ValueError:
        how = "value"
    return how


def summed_index(a):
    try:
        a.sum(axis=5)                   #@shape-error
        how = "ok"
    except IndexError:
        how = "index"
    return how


def summed_value(a):
    try:
        a.sum(axis=5)                   #@shape-error
        how = "ok"
    except ValueError:
        how = "value"
    return how


if __name__ == "__main__":
    r = reshaped(np.zeros((2, 3)))
    #@value r: exactly={"value"}
    p = product(np.zeros((2, 3)))
    #@value p: exactly={"value"}
    si = summed_index(np.zeros((2, 3)))
    #@value si: exactly={"index"}
    sv = summed_value(np.zeros((2, 3)))
    #@value sv: includes={"value"}

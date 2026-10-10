# np.where (three-argument: broadcast), boolean masks (unknown length), and a definite shape mismatch
import numpy as np


def clip_neg(x):
    return np.where(x > 0, x, 0.0)


def bad_add():
    a = np.zeros((2, 3))
    b = np.zeros((4,))
    return a + b  #@shape-error


if __name__ == "__main__":
    c = clip_neg(np.ones((3, 2)))  #@shape c: [3, 2]
    x = np.arange(10)
    pos = x[x > 4]  #@shape pos: [?]
    try:
        bad_add()
    except (ValueError, RuntimeError, IndexError, TypeError, AssertionError):
        pass

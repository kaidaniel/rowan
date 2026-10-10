# numpy reductions with axis / keepdims / tuples of axes / no axis (a scalar), argmax, cumsum, the module functions and methods
import numpy as np


def stats(x):
    B, T, C = x.shape
    s = x.sum(axis=1)  #@shape s: [x.shape[0], x.shape[2]]
    m = x.mean(axis=-1, keepdims=True)  #@shape m: [x.shape[0], x.shape[1], 1]
    mx = np.max(x, axis=0)  #@shape mx: [x.shape[1], x.shape[2]]
    st = x.std(axis=(0, 2))  #@shape st: [x.shape[1]]
    a = np.argmax(x, axis=2)  #@shape a: [x.shape[0], x.shape[1]]
    c = np.cumsum(x, axis=1)  #@shape c: [x.shape[0], x.shape[1], x.shape[2]]
    tot = x.sum()  #@shape tot: []
    return s, m, mx, st, a, c, tot


if __name__ == "__main__":
    s, m, mx, st, a, c, tot = stats(np.ones((2, 3, 4)))  #@shape s: [2, 4]; m: [2, 3, 1]; mx: [3, 4]; st: [3]; a: [2, 3]; c: [2, 3, 4]

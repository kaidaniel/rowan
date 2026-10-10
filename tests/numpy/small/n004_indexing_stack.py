# numpy indexing and slicing (ints, slices, None, Ellipsis), concatenate / stack / vstack / hstack along axes
import numpy as np


def pieces(x):
    B, T, C = x.shape
    first = x[0]  #@shape first: [x.shape[1], x.shape[2]]
    col = x[:, :, 0]  #@shape col: [x.shape[0], x.shape[1]]
    tail = x[:, 1:]  #@shape tail: [x.shape[0], x.shape[1] - 1, x.shape[2]]
    e = x[..., 0]  #@shape e: [x.shape[0], x.shape[1]]
    return first, col, tail, e


def joins(a, b):
    N, D = a.shape
    M, D2 = b.shape
    cat = np.concatenate([a, b], axis=0)  #@shape cat: [a.shape[0] + b.shape[0], a.shape[1]]
    st = np.stack([a, a])  #@shape st: [2, a.shape[0], a.shape[1]]
    return cat, st


if __name__ == "__main__":
    first, col, tail, e = pieces(np.zeros((2, 3, 4)))  #@shape first: [3, 4]; col: [2, 3]; tail: [2, 2, 4]; e: [2, 3]
    cat, st = joins(np.zeros((2, 5)), np.zeros((3, 5)))  #@shape cat: [5, 5]; st: [2, 2, 5]
    v = np.vstack([np.zeros((2, 5)), np.zeros((1, 5))])  #@shape v: [3, 5]
    h = np.hstack([np.zeros((2, 5)), np.zeros((2, 1))])  #@shape h: [2, 6]

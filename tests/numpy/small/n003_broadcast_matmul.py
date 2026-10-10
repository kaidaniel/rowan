# numpy broadcasting (literal 1 dims, trailing alignment), matmul / dot / @, outer products via None indexing
import numpy as np


def center(x, mu):
    return x - mu


def project(x, w):
    B, D = x.shape
    D2, K = w.shape
    y = x @ w  #@shape y: [x.shape[0], w.shape[1]]
    z = np.dot(x, w)  #@shape z: [x.shape[0], w.shape[1]]
    o = x[:, :, None] * w[None, :, :]  #@shape o: [x.shape[0], x.shape[1], w.shape[1]]
    return y, z, o


if __name__ == "__main__":
    c = center(np.ones((5, 3)), np.ones(3))  #@shape c: [5, 3]
    c2 = center(np.ones((5, 1)), np.ones((1, 4)))  #@shape c2: [5, 4]
    y, z, o = project(np.ones((4, 3)), np.ones((3, 2)))  #@shape y: [4, 2]; z: [4, 2]; o: [4, 3, 2]
    v = np.ones(3) @ np.ones(3)  #@shape v: []

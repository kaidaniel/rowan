# numpy creation (zeros / ones / full / arange / linspace / eye / array literals), reshape with -1, transpose / .T, flatten, expand_dims / squeeze
import numpy as np


def grid(h, w):
    z = np.zeros((h, w))  #@shape z: [h, w]
    o = np.ones((2, h, w))  #@shape o: [2, h, w]
    f = np.full((h, 3), 7.0)  #@shape f: [h, 3]
    return z, o, f


def shapes(x):
    B, T, C = x.shape
    r = x.reshape(B, -1)  #@shape r: [x.shape[0], x.shape[1]*x.shape[2]]
    t = x.transpose(0, 2, 1)  #@shape t: [x.shape[0], x.shape[2], x.shape[1]]
    tt = r.T  #@shape tt: [x.shape[1]*x.shape[2], x.shape[0]]
    fl = x.flatten()  #@shape fl: [x.shape[0]*x.shape[1]*x.shape[2]]
    e = np.expand_dims(r, 0)  #@shape e: [1, x.shape[0], x.shape[1]*x.shape[2]]
    sq = e.squeeze(0)  #@shape sq: [x.shape[0], x.shape[1]*x.shape[2]]
    return r, t, tt, fl, e, sq


if __name__ == "__main__":
    z, o, f = grid(3, 4)  #@shape z: [3, 4]; o: [2, 3, 4]; f: [3, 3]
    a = np.arange(12)  #@shape a: [12]
    m = a.reshape(3, 4)  #@shape m: [3, 4]
    l = np.linspace(0, 1, 5)  #@shape l: [5]
    i = np.eye(4)  #@shape i: [4, 4]
    lit = np.array([[1, 2, 3], [4, 5, 6]])  #@shape lit: [2, 3]
    r, t, tt, fl, e, sq = shapes(np.zeros((2, 3, 4)))  #@shape r: [2, 12]; t: [2, 4, 3]; tt: [12, 2]; fl: [24]; e: [1, 2, 12]; sq: [2, 12]

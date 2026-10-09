# numpy arrays next to tensors: a.shape ints, from_numpy / as_tensor, numpy methods sharing torch names
import numpy as np
import torch


def concrete():
    a = np.zeros((3, 4))
    r, c = a.shape  #@dim r: 3; c: 4
    t = torch.from_numpy(a).float()  #@shape t: [3, 4]
    z = t @ torch.randn(4, 2)  #@shape z: [3, 2]
    s = a.sum(axis=0)
    ts = torch.as_tensor(s)  #@shape ts: [4]
    at = torch.tensor(a.T.copy())  #@shape at: [4, 3]
    m = torch.zeros(a.shape[0] * 2, c)  #@shape m: [6, 4]
    b = np.ones(5)
    tb = torch.from_numpy(b)  #@shape tb: [5]
    back = t.numpy()
    n = back.shape[1]  #@dim n: 4
    return z, ts, at, m, tb


def from_array(arr, x):
    B, C = x.shape
    t = torch.from_numpy(arr).float()
    y = x * 2  #@shape y: [x.shape[0], x.shape[1]]
    k = arr.shape[0]
    w = torch.zeros(k, C)  #@shape w: [_, x.shape[1]]
    return t, w


if __name__ == "__main__":
    concrete()
    t, w = from_array(np.zeros((7, 2), dtype=np.float32), torch.randn(3, 5))  #@shape t: [7, 2]; w: [7, 5]

# in-place index stores (x[:, 0] = 0, x[mask] = v, pe[:, 0::2] = sin(...), +=) keep the tensor's shape
import math
import torch


def stores(x):
    B, T, C = x.shape
    x[:, 0] = 0
    m = x > 0
    x[m] = 1.0
    x[..., -1] = x[..., 0]
    x[0] = torch.zeros(C)
    x[:, :, 0] += 1.0
    out = x  #@shape out: [x.shape[0], x.shape[1], x.shape[2]]
    #@requires none
    return out


def pos_enc(T, d):
    pe = torch.zeros(T, d)  #@shape pe: [T, d]
    pos = torch.arange(T).unsqueeze(1).float()  #@shape pos: [T, 1]
    div = torch.exp(torch.arange(0, d, 2).float() * (-math.log(10000.0) / d))  #@shape div: [_]
    pe[:, 0::2] = torch.sin(pos * div)
    pe[:, 1::2] = torch.cos(pos * div)
    pe1 = pe.unsqueeze(0)  #@shape pe1: [1, T, d]
    return pe1


def masked(x):
    y = x.clone()  #@shape y: [*x.shape]
    y[y < 0] = 0.0
    y[..., 0] = 1.0
    z = y  #@shape z: [*x.shape]
    return z


if __name__ == "__main__":
    a = stores(torch.randn(2, 3, 4))  #@shape a: [2, 3, 4]
    p = pos_enc(5, 6)  #@shape p: [1, 5, 6]
    z = masked(torch.randn(3, 2))  #@shape z: [3, 2]

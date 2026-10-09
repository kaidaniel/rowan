# torch.arange with 1, 2 and 3 arguments: lengths for literal bounds, symbolic bounds, bounds from a shape
import torch


def concrete():
    a = torch.arange(5)  #@shape a: [5]
    b = torch.arange(2, 7)  #@shape b: [5]
    c = torch.arange(0, 10, 3)  #@shape c: [4]
    d = torch.arange(1, 10, 2)  #@shape d: [5]
    e = torch.arange(10, 0, -2)  #@shape e: [5]
    f = torch.arange(0.0, 1.0, 0.25)  #@shape f: [4]
    return a, b, c, d, e, f


def symbolic(T, a, b):
    p = torch.arange(T)  #@shape p: [T]
    q = torch.arange(a, b)  #@shape q: [b - a]
    r = torch.arange(0, T, dtype=torch.long)  #@shape r: [T]
    return p, q, r


def from_shape(x):
    B, T = x.shape
    #@requires x.dim() == 2
    pos = torch.arange(T)  #@shape pos: [x.shape[1]]
    pos1 = torch.arange(1, T + 1)  #@shape pos1: [x.shape[1]]
    return pos, pos1


if __name__ == "__main__":
    concrete()
    symbolic(8, 3, 10)
    from_shape(torch.zeros(2, 6))

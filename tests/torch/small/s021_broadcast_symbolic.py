# broadcasting with known ranks and symbolic dims: [B,T,C]+[C], [T,1] factors, max(a, b) for undecided pairs
import torch


def bias(x):
    B, T, C = x.shape
    #@requires x.dim() == 3
    b = torch.zeros(C)
    y = x + b  #@shape y: [x.shape[0], x.shape[1], x.shape[2]]
    #@requires none
    g = torch.ones(T, 1)
    z = x * g  #@shape z: [x.shape[0], x.shape[1], x.shape[2]]
    #@requires none
    return y, z


def two(x, y):
    a, b = x.shape
    #@requires x.dim() == 2
    c, d = y.shape
    #@requires y.dim() == 2
    z = x + y  #@shape z: [max(x.shape[0], y.shape[0]), max(x.shape[1], y.shape[1])]
    return z


def lit(x):
    B, C = x.shape
    #@requires x.dim() == 2
    z = x + torch.ones(B, 1)  #@shape z: [x.shape[0], x.shape[1]]
    w = torch.zeros(B, 1) + torch.zeros(1, C)  #@shape w: [x.shape[0], x.shape[1]]
    v = torch.zeros(B, 1, C) + torch.zeros(4, C)  #@shape v: [x.shape[0], 4, x.shape[1]]
    u = x + torch.ones(5)  #@shape u: [x.shape[0], max(x.shape[1], 5)]
    return z, w, v, u


def bad(n):
    y = torch.zeros(n, 2) + torch.zeros(n, 3)  #@shape-error
    return y


if __name__ == "__main__":
    bias(torch.randn(2, 3, 4))
    two(torch.randn(2, 3), torch.randn(1, 3))
    two(torch.randn(4, 1), torch.randn(4, 6))
    lit(torch.randn(3, 5))
    lit(torch.randn(3, 1))
    try:
        bad(3)
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

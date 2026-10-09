# operands of unknown shape: x + y broadcasts the two shapes, an unknown s in x * s may be a tensor, scalars keep x's shape
import math

import torch


def two(x, y):
    z = x + y  #@shape z: [*broadcast(x.shape, y.shape)]
    w = torch.mul(x, y)  #@shape w: [*broadcast(x.shape, y.shape)]
    return z, w


def scale(x, s):
    y = x * s  #@shape y: [*broadcast(x.shape, s.shape)]
    return y


def scalars(x):
    a = x + 1  #@shape a: [*x.shape]
    b = 2.0 * x - 0.5  #@shape b: [*x.shape]
    d = x * x.shape[0]  #@shape d: [*x.shape]
    e = x / math.sqrt(x.shape[-1])  #@shape e: [*x.shape]
    f = x / x.size(-1) ** 0.5  #@shape f: [*x.shape]
    return a, b, d, e, f


if __name__ == "__main__":
    two(torch.randn(2, 3), torch.randn(3))
    two(torch.randn(4, 1, 2), torch.randn(5, 1))
    scale(torch.randn(2, 3), torch.randn(3))
    scale(torch.randn(4), torch.tensor(2.0))
    scalars(torch.randn(2, 4))

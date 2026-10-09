# x @ y on parameters: symbolic shapes, conditional on ranks until the ranks are known
import torch


def mm_unknown(x, y):
    z = x @ y  #@shape z: [*broadcast(x.shape[:-2], y.shape[:-2]), x.shape[-2], y.shape[-1]] if x.dim() >= 2, y.dim() >= 2
    return z


def mm_known(x, y):
    B, T, C = x.shape  #@dim B: x.shape[0]; T: x.shape[1]; C: x.shape[2]
    z = x @ y  #@shape z: [*broadcast([x.shape[0]], y.shape[:-2]), x.shape[1], y.shape[-1]] if y.dim() >= 2
    #@requires x.shape[2] == y.shape[-2] if y.dim() >= 2
    w = torch.randn(C, 7)  #@shape w: [x.shape[2], 7]
    h = x @ w  #@shape h: [x.shape[0], x.shape[1], 7]
    #@requires none
    return z, h


def concrete():
    a = torch.randn(4, 3)
    b = torch.randn(3, 5)
    c = a @ b  #@shape c: [4, 5]
    v = torch.randn(3)
    d = a @ v  #@shape d: [4]
    e = v @ v  #@shape e: []
    f = torch.randn(2, 1, 4, 3) @ torch.randn(6, 3, 5)  #@shape f: [2, 6, 4, 5]
    return c, d, e, f


if __name__ == "__main__":
    mm_unknown(torch.randn(2, 3, 4), torch.randn(4, 5))
    mm_unknown(torch.randn(3), torch.randn(3))
    mm_known(torch.randn(2, 3, 4), torch.randn(4, 5))
    concrete()

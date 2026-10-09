# permute with varargs, a tuple and a list; the permutation fixes the rank
import torch


def perm_unknown(x):
    y = x.permute(0, 2, 1)  #@shape y: [x.shape[0], x.shape[2], x.shape[1]]
    #@requires x.dim() == 3
    z = y.permute((2, 0, 1))  #@shape z: [x.shape[1], x.shape[0], x.shape[2]]
    #@requires none
    w = x.permute(-1, 0, 1)  #@shape w: [x.shape[2], x.shape[0], x.shape[1]]
    #@requires none
    return z, w


def nchw(x):
    N, H, W, C = x.shape  #@requires x.dim() == 4
    y = x.permute(0, 3, 1, 2)  #@shape y: [x.shape[0], x.shape[3], x.shape[1], x.shape[2]]
    z = x.permute([0, -1, -3, -2])  #@shape z: [x.shape[0], x.shape[3], x.shape[1], x.shape[2]]
    back = y.permute(0, 2, 3, 1)  #@shape back: [x.shape[0], x.shape[1], x.shape[2], x.shape[3]]
    f = torch.permute(x, (3, 2, 1, 0))  #@shape f: [x.shape[3], x.shape[2], x.shape[1], x.shape[0]]
    return z, back, f


def concrete():
    a = torch.zeros(2, 3, 4, 5).permute(1, 0, 3, 2)  #@shape a: [3, 2, 5, 4]
    return a


def bad_rank():
    a = torch.randn(2, 3, 4)
    return a.permute(0, 1)  #@shape-error


if __name__ == "__main__":
    perm_unknown(torch.randn(2, 3, 4))
    perm_unknown(torch.randn(5, 1, 7))
    nchw(torch.randn(2, 8, 6, 3))
    concrete()
    try:
        bad_rank()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

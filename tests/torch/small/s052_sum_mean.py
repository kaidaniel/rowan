# sum / mean with dim, keepdim, dim tuples and lists, and no dim (a scalar)
import torch


def red_known(x):
    B, T, C = x.shape  #@requires x.dim() == 3
    a = x.sum(dim=1)  #@shape a: [x.shape[0], x.shape[2]]
    b = x.sum(1, keepdim=True)  #@shape b: [x.shape[0], 1, x.shape[2]]
    c = x.mean(dim=(1, 2))  #@shape c: [x.shape[0]]
    d = x.sum()  #@shape d: []
    e = x.mean(-1)  #@shape e: [x.shape[0], x.shape[1]]
    f = torch.sum(x, dim=[0, -1], keepdim=True)  #@shape f: [1, x.shape[1], 1]
    #@requires none
    g = torch.mean(x, 0)  #@shape g: [x.shape[1], x.shape[2]]
    return a, b, c, d, e, f, g


def red_unknown(x):
    a = x.sum(-1)  #@shape a: [*x.shape[:-1]]
    b = x.mean(dim=-1, keepdim=True)  #@shape b: [*x.shape[:-1], 1]
    d = x.mean()  #@shape d: []
    c = x.sum(dim=2)  #@shape c: [x.shape[0], x.shape[1], *x.shape[3:]]
    #@requires x.dim() >= 3
    return a, b, c, d


def bad_dim(x):
    B, T, C = x.shape
    return x.sum(dim=3)  #@shape-error


if __name__ == "__main__":
    red_known(torch.randn(2, 3, 4))
    red_unknown(torch.randn(2, 3, 4))
    red_unknown(torch.randn(2, 3, 4, 5))
    try:
        bad_dim(torch.randn(2, 3, 4))
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

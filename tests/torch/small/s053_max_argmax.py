# max/min with a dim return (values, indices); without a dim a scalar; argmax, amax, amin
import torch


def maxes(x):
    B, T, C = x.shape  #@requires x.dim() == 3
    v, i = x.max(dim=1)  #@shape v: [x.shape[0], x.shape[2]]; i: [x.shape[0], x.shape[2]]
    r = x.max(dim=-1, keepdim=True)
    rv = r.values  #@shape rv: [x.shape[0], x.shape[1], 1]
    ri = r.indices  #@shape ri: [x.shape[0], x.shape[1], 1]
    m = x.max()  #@shape m: []
    a = x.argmax()  #@shape a: []
    a1 = x.argmax(dim=-1)  #@shape a1: [x.shape[0], x.shape[1]]
    a2 = torch.argmax(x, 1, keepdim=True)  #@shape a2: [x.shape[0], 1, x.shape[2]]
    mn, mi = torch.min(x, 0)  #@shape mn: [x.shape[1], x.shape[2]]; mi: [x.shape[1], x.shape[2]]
    am = x.amax(dim=(-2, -1))  #@shape am: [x.shape[0]]
    an = torch.amin(x, dim=1)  #@shape an: [x.shape[0], x.shape[2]]
    #@requires none
    return v, i, rv, ri, m, a, a1, a2, mn, mi, am, an


def max_unknown(x):
    v, i = x.max(-1)  #@shape v: [*x.shape[:-1]]; i: [*x.shape[:-1]]
    a = x.argmax(dim=-1)  #@shape a: [*x.shape[:-1]]
    m = x.max()  #@shape m: []
    return v, i, a, m


def bad_dim(x):
    B, T, C = x.shape
    return x.max(dim=3)  #@shape-error


if __name__ == "__main__":
    maxes(torch.randn(2, 3, 4))
    max_unknown(torch.randn(2, 3))
    max_unknown(torch.randn(5))
    try:
        bad_dim(torch.randn(2, 3, 4))
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

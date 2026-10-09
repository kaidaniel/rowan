# view / reshape with -1: the missing dim is divided out symbolically when the division is exact
import torch


def flat_tail(x):
    B, T, C = x.shape  #@requires x.dim() == 3
    a = x.view(B, -1)  #@shape a: [x.shape[0], x.shape[1]*x.shape[2]]
    #@requires none
    b = x.view(-1, C)  #@shape b: [x.shape[0]*x.shape[1], x.shape[2]]
    c = x.reshape(B * T, -1)  #@shape c: [x.shape[0]*x.shape[1], x.shape[2]]
    #@requires none
    d = x.reshape(-1)  #@shape d: [x.shape[0]*x.shape[1]*x.shape[2]]
    e = a.view(B, T, -1)  #@shape e: [x.shape[0], x.shape[1], x.shape[2]]
    f = d.view(-1, T * C)  #@shape f: [x.shape[0], x.shape[1]*x.shape[2]]
    return a, b, c, d, e, f


def flat_unknown_rank(x):
    y = x.view(-1)  #@shape y: [?]
    #@requires none
    return y


def concrete():
    t = torch.arange(24)
    a = t.view(2, -1, 4)  #@shape a: [2, 3, 4]
    b = t.reshape(-1, 6)  #@shape b: [4, 6]
    c = a.view(-1)  #@shape c: [24]
    return a, b, c


def bad_count():
    t = torch.arange(24)
    return t.view(5, -1)  #@shape-error


if __name__ == "__main__":
    flat_tail(torch.randn(2, 3, 4))
    flat_tail(torch.randn(5, 1, 6))
    flat_unknown_rank(torch.randn(2, 3, 4))
    flat_unknown_rank(torch.randn(7))
    concrete()
    try:
        bad_count()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

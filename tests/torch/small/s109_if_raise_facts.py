# `if x.dim() != 3: raise` and `if x.size(-1) != d: raise` give facts on the fall-through path
import torch


def check_rank(x):
    if x.dim() != 3:
        raise ValueError("want a 3-D tensor")
    B, T, C = x.shape
    #@requires none
    y = x.transpose(1, 2)  #@shape y: [x.shape[0], x.shape[2], x.shape[1]]
    #@requires none
    return y


def check_last(x, d):
    if x.size(-1) != d:
        raise ValueError("last dim")
    y = x * 2  #@shape y: [*x.shape[:-1], d]
    return y


def check_both(x, n):
    if x.ndim != 2 or x.shape[0] != n:
        raise ValueError("want (n, k)")
    y = x.t()  #@shape y: [x.shape[1], n]
    #@requires none
    return y


if __name__ == "__main__":
    y = check_rank(torch.randn(2, 3, 4))  #@shape y: [2, 4, 3]
    z = check_last(torch.randn(5, 7), 7)  #@shape z: [5, 7]
    w = check_both(torch.randn(3, 6), 3)  #@shape w: [6, 3]
    try:
        check_rank(torch.randn(3))
    except ValueError:
        pass

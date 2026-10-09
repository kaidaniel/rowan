# rank-dependent branches: the else / elif branches must not inherit the then-branch's rank fact
import torch


def by_rank(x):
    if x.ndim == 3:
        y = x.mean(dim=1)  #@shape y: [x.shape[0], x.shape[2]]
        #@requires none
    elif x.dim() == 2:
        y = x.t()  #@shape y: [x.shape[1], x.shape[0]]
        #@requires none
    else:
        y = x.transpose(1, 2)  #@shape y: [x.shape[0], x.shape[2], x.shape[1], *x.shape[3:]]
        #@requires x.dim() >= 3
    return y


def ensure_batched(x):
    if x.dim() == 2:
        xb = x.unsqueeze(0)  #@shape xb: [1, x.shape[0], x.shape[1]]
    else:
        xb = x  #@shape xb: [*x.shape]
        s = x.sum(dim=-1)  #@shape s: [*x.shape[:-1]]
    return xb


def not_two(x):
    if x.dim() == 2:
        return x
    z = x.flatten(1)  #@shape z: [x.shape[0], _]
    return z


if __name__ == "__main__":
    a = by_rank(torch.randn(2, 3, 4))  #@shape a: [2, 4]
    b = by_rank(torch.randn(2, 3))  #@shape b: [3, 2]
    c = by_rank(torch.randn(2, 3, 4, 5))  #@shape c: [2, 4, 3, 5]
    d = ensure_batched(torch.randn(3, 4))  #@shape d: [1, 3, 4]
    e = ensure_batched(torch.randn(2, 3, 4))  #@shape e: [2, 3, 4]
    f = not_two(torch.randn(2, 3, 4))  #@shape f: [2, 12]
    g = not_two(torch.randn(2, 3))  #@shape g: [2, 3]

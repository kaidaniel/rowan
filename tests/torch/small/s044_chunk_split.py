# chunk / split into tuples: nanoGPT's q, k, v = qkv.split(n_embd, dim=2), split by a size list
import torch


def qkv_split(x):
    B, T, C = x.shape  #@requires x.dim() == 3
    w = torch.randn(C, 3 * C)
    qkv = x @ w  #@shape qkv: [x.shape[0], x.shape[1], 3*x.shape[2]]
    #@requires none
    q, k, v = qkv.split(C, dim=2)  #@shape q: [x.shape[0], x.shape[1], x.shape[2]]; v: [x.shape[0], x.shape[1], x.shape[2]]
    #@requires none
    a, b, c = qkv.chunk(3, dim=-1)  #@shape a: [x.shape[0], x.shape[1], x.shape[2]]; c: [x.shape[0], x.shape[1], x.shape[2]]
    #@requires none
    return q, k, v, a, b, c


def nanogpt_split(x, n_embd):
    B, T, C = x.shape  #@requires x.dim() == 3
    y = x @ torch.randn(C, 3 * n_embd)  #@shape y: [x.shape[0], x.shape[1], 3*n_embd]
    q, k, v = y.split(n_embd, dim=2)  #@shape q: [x.shape[0], x.shape[1], n_embd]; k: [x.shape[0], x.shape[1], n_embd]
    #@requires none
    return q, k, v


def split_sizes(x):
    a, b = x.split([2, 3], dim=-1)  #@shape a: [*x.shape[:-1], 2]; b: [*x.shape[:-1], 3]
    #@requires x.shape[-1] == 5
    return a, b


def head_tail(x):
    h, t = torch.split(x, [1, x.shape[0] - 1])  #@shape h: [1, *x.shape[1:]]; t: [x.shape[0] - 1, *x.shape[1:]]
    #@requires none
    return h, t


def concrete():
    parts = torch.arange(10).chunk(3)
    n = len(parts)  #@dim n: 3
    last = parts[2]  #@shape last: [2]
    s = torch.zeros(5, 4).split(2)
    s2 = s[2]  #@shape s2: [1, 4]
    return n, last, s2


def bad_sizes():
    a, b = torch.zeros(4, 5).split([2, 2], dim=1)  #@shape-error
    return a, b


if __name__ == "__main__":
    qkv_split(torch.randn(2, 3, 4))
    nanogpt_split(torch.randn(2, 3, 4), 6)
    split_sizes(torch.randn(2, 5))
    split_sizes(torch.randn(5))
    head_tail(torch.randn(4, 3))
    concrete()
    try:
        bad_sizes()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

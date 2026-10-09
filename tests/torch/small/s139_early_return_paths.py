# a function returning different shapes on different paths (early return): callers with a decided flag get one path, an unknown flag the join; None on one path
import torch


def encode(x, pool):
    B, T, C = x.shape
    h = torch.tanh(x)
    if pool:
        return h.mean(dim=1), T
    return h, T


def maybe_proj(x, w):
    if w is None:
        return None
    return x @ w


def use(x, flag):
    B, T, C = x.shape
    a, n = encode(x, True)  #@shape a: [x.shape[0], x.shape[2]]
    #@dim n: x.shape[1]
    b, m = encode(x, False)  #@shape b: [x.shape[0], x.shape[1], x.shape[2]]
    # (was `[*, x.shape[2]]`: the join of [B, C] and [B, T, C] keeps both trailing positions,
    # unknown where they differ — also true, and more precise)
    c, k = encode(x, flag)  #@shape c: [*, ?, x.shape[2]]
    #@dim k: x.shape[1]
    p = maybe_proj(x, None)  #@shape p: none
    q = maybe_proj(x, torch.randn(C, 2))  #@shape q: [x.shape[0], x.shape[1], 2]
    #@requires none
    return a, b, c, q


if __name__ == "__main__":
    a, b, c, q = use(torch.randn(2, 3, 4), True)  #@shape a: [2, 4]; b: [2, 3, 4]; c: [2, 4]; q: [2, 3, 2]
    a2, b2, c2, q2 = use(torch.randn(2, 3, 4), False)  #@shape c2: [2, 3, 4]

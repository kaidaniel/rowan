# squeeze: literal 1 dims go; a symbolic dim may or may not be 1, so the rank joins (common trailing dims)
import torch


def squeeze_literal(x):
    B, C = x.shape  #@requires x.dim() == 2
    a = x.unsqueeze(1)
    b = a.squeeze(1)  #@shape b: [x.shape[0], x.shape[1]]
    c = x[:, None, :, None]  #@shape c: [x.shape[0], 1, x.shape[1], 1]
    d = c.squeeze(-1)  #@shape d: [x.shape[0], 1, x.shape[1]]
    e = torch.squeeze(c, 1).squeeze(2)  #@shape e: [x.shape[0], x.shape[1]]
    return b, d, e


def squeeze_symbolic(x):
    B, T, C = x.shape  #@requires x.dim() == 3
    # (was `[*, x.shape[2]]`: the join of [B, T, C] and [B, C] keeps both trailing positions,
    # unknown where they differ — also true, and more precise)
    y = x.squeeze(1)  #@shape y: [*, ?, x.shape[2]]
    z = x.squeeze()  #@shape z: [*]
    return y, z


def concrete():
    t = torch.zeros(2, 1, 3, 1)
    a = t.squeeze()  #@shape a: [2, 3]
    b = t.squeeze(0)  #@shape b: [2, 1, 3, 1]
    c = t.squeeze(-1)  #@shape c: [2, 1, 3]
    d = t.squeeze((1, 3))  #@shape d: [2, 3]
    return a, b, c, d


def bad_dim():
    t = torch.zeros(2, 1)
    return t.squeeze(2)  #@shape-error


if __name__ == "__main__":
    squeeze_literal(torch.randn(3, 4))
    squeeze_symbolic(torch.randn(2, 1, 4))
    squeeze_symbolic(torch.randn(2, 3, 4))
    concrete()
    try:
        bad_dim()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

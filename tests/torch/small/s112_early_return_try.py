# early returns joined at the caller, and try/except around shape ops (a caught definite mismatch)
import torch


def head_or_all(x, k):
    B, T, C = x.shape
    if k is None:
        return x
    y = x[:, :k]  #@shape y: [x.shape[0], min(k, x.shape[1]), x.shape[2]]
    return y


def caller(x, k):
    B, T, C = x.shape
    a = head_or_all(x, None)  #@shape a: [x.shape[0], x.shape[1], x.shape[2]]
    b = head_or_all(x, 2)  #@shape b: [x.shape[0], min(2, x.shape[1]), x.shape[2]]
    c = head_or_all(x, k)  #@shape c: [x.shape[0], ?, x.shape[2]]
    return a, b, c


def flat(x):
    B, T, C = x.shape
    try:
        y = x.view(B, T * C)
    except RuntimeError:
        y = x.reshape(B, T * C)  #@shape y: [x.shape[0], x.shape[1]*x.shape[2]]
    out = y  #@shape out: [x.shape[0], x.shape[1]*x.shape[2]]
    return out


def fallback():
    a = torch.randn(4, 6)
    try:
        b = a @ torch.randn(4, 6)  #@shape-error
    except RuntimeError:
        b = a @ torch.randn(6, 4)  #@shape b: [4, 4]
    c = b + 1  #@shape c: [4, 4]
    return c


if __name__ == "__main__":
    a, b, c = caller(torch.randn(2, 5, 3), 4)  #@shape a: [2, 5, 3]; b: [2, 2, 3]; c: [2, 4, 3]
    f = flat(torch.randn(2, 5, 3).transpose(0, 1))  #@shape f: [5, 6]
    g = fallback()  #@shape g: [4, 4]

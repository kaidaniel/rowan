# basic indexing: ints (also negative), slices with literal / symbolic bounds, None, Ellipsis
import torch


def idx_unknown(x):
    a = x[0]  #@shape a: [*x.shape[1:]]
    b = x[-1]  #@shape b: [*x.shape[1:]]
    c = x[..., 0]  #@shape c: [*x.shape[:-1]]
    d = x[None]  #@shape d: [1, *x.shape]
    f = x[..., None]  #@shape f: [*x.shape, 1]
    g = x[:, -1, :]  #@shape g: [x.shape[0], *x.shape[2:]]
    #@requires x.dim() >= 3
    e = x[:, None]  #@shape e: [x.shape[0], 1, *x.shape[1:]]
    return a, b, c, d, e, f, g


def idx_slices(x, T, a, b):
    B, L, C = x.shape  #@requires x.dim() == 3
    s1 = x[:, :T]  #@shape s1: [x.shape[0], min(T, x.shape[1]), x.shape[2]]
    s2 = x[:, 1:]  #@shape s2: [x.shape[0], x.shape[1] - 1, x.shape[2]]
    s3 = x[:, a:b]  #@shape s3: [x.shape[0], ?, x.shape[2]]
    s4 = x[:, 2:5]  #@shape s4: [x.shape[0], min(5, x.shape[1]) - 2, x.shape[2]]
    s5 = x[:, -1]  #@shape s5: [x.shape[0], x.shape[2]]
    s6 = x[:, :, :4]  #@shape s6: [x.shape[0], x.shape[1], min(4, x.shape[2])]
    #@requires none
    s7 = x[0, :, 1]  #@shape s7: [x.shape[1]]
    return s1, s2, s3, s4, s5, s6, s7


def concrete():
    t = torch.zeros(4, 5, 6)
    a = t[1:3, ::2]  #@shape a: [2, 3, 6]
    b = t[..., 1:]  #@shape b: [4, 5, 5]
    c = t[:, None, 0]  #@shape c: [4, 1, 6]
    d = t[-2:, 3, :10]  #@shape d: [2, 6]
    return a, b, c, d


def bad_too_many():
    t = torch.zeros(3, 4)
    return t[:, :, 0]  #@shape-error


if __name__ == "__main__":
    idx_unknown(torch.randn(2, 3, 4))
    idx_unknown(torch.randn(2, 3, 4, 5))
    idx_slices(torch.randn(2, 6, 5), 3, 1, 4)
    idx_slices(torch.randn(2, 6, 3), 10, 0, 2)
    concrete()
    try:
        bad_too_many()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

# unflatten(dim, sizes): one dim becomes several; round trips through flatten
import torch


def unflat(x, n_head):
    B, T, C = x.shape  #@requires x.dim() == 3
    a = x.unflatten(2, (n_head, C // n_head))  #@shape a: [x.shape[0], x.shape[1], n_head, x.shape[2] // n_head]
    d = x.unflatten(0, (1, B))  #@shape d: [1, x.shape[0], x.shape[1], x.shape[2]]
    #@requires none
    e = x.flatten(0, 1).unflatten(0, (B, T))  #@shape e: [x.shape[0], x.shape[1], x.shape[2]]
    #@requires none
    f = torch.unflatten(x, 1, (T, 1))  #@shape f: [x.shape[0], x.shape[1], 1, x.shape[2]]
    #@requires none
    g = x.unflatten(-1, [C, 1])  #@shape g: [x.shape[0], x.shape[1], x.shape[2], 1]
    return a, d, e, f, g


def concrete():
    t = torch.zeros(2, 12)
    a = t.unflatten(1, (3, 4))  #@shape a: [2, 3, 4]
    b = t.unflatten(1, (-1, 4))  #@shape b: [2, 3, 4]
    c = t.unflatten(-1, (2, 3, 2))  #@shape c: [2, 2, 3, 2]
    return a, b, c


def bad_sizes():
    t = torch.zeros(2, 12)
    return t.unflatten(1, (5, 4))  #@shape-error


if __name__ == "__main__":
    unflat(torch.randn(2, 3, 12), 4)
    unflat(torch.randn(1, 5, 6), 6)
    concrete()
    try:
        bad_sizes()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

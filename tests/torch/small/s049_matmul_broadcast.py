# matmul: batch dims broadcast, 1-D operands are promoted then dropped; requirements on the inner dims
import torch


def batch_times_matrix(x, w):
    B, T, C = x.shape  #@requires x.dim() == 3
    K, N = w.shape  #@requires w.dim() == 2
    y = x @ w  #@shape y: [x.shape[0], x.shape[1], w.shape[1]]
    #@requires x.shape[2] == w.shape[0]
    z = torch.matmul(y, w.t())  #@shape z: [x.shape[0], x.shape[1], x.shape[2]]
    #@requires none
    return y, z


def vec_cases(v, m):
    (n,) = v.shape  #@requires v.dim() == 1
    r, c = m.shape  #@requires m.dim() == 2
    a = m @ v  #@shape a: [m.shape[0]]
    #@requires m.shape[1] == v.shape[0]
    d = v @ v  #@shape d: []
    #@requires none
    return a, d


def bcast(x, y):
    B, M, K = x.shape  #@requires x.dim() == 3
    H, G, K2, N = y.shape  #@requires y.dim() == 4
    z = x @ y  #@shape z: [y.shape[0], max(x.shape[0], y.shape[1]), x.shape[1], y.shape[3]]
    return z


def unknown_left(x):
    y = x @ torch.randn(4, 5)  #@shape y: [*x.shape[:-2], x.shape[-2], 5] if x.dim() >= 2
    return y


def concrete():
    a = torch.randn(5, 1, 2, 3) @ torch.randn(4, 3, 6)  #@shape a: [5, 4, 2, 6]
    b = torch.randn(3) @ torch.randn(2, 3, 4)  #@shape b: [2, 4]
    c = torch.matmul(torch.randn(2, 3, 4), torch.randn(4))  #@shape c: [2, 3]
    return a, b, c


def bad_inner():
    return torch.randn(2, 3) @ torch.randn(4, 5)  #@shape-error


if __name__ == "__main__":
    batch_times_matrix(torch.randn(2, 3, 4), torch.randn(4, 5))
    vec_cases(torch.randn(3), torch.randn(2, 3))
    bcast(torch.randn(1, 2, 3), torch.randn(4, 5, 3, 6))
    bcast(torch.randn(5, 2, 3), torch.randn(4, 5, 3, 6))
    unknown_left(torch.randn(2, 3, 4))
    unknown_left(torch.randn(4))
    concrete()
    try:
        bad_inner()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

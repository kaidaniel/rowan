# unpacking a shape requires its rank (exact, or >= for a prefix slice); a decided wrong rank is a shape error
import torch


def unpack3(x):
    B, T, C = x.shape  #@dim B: x.shape[0]; T: x.shape[1]; C: x.shape[2]
    #@requires x.dim() == 3
    y = torch.zeros(C, T, B)  #@shape y: [x.shape[2], x.shape[1], x.shape[0]]
    return y


def unpack_size(x):
    N, D = x.size()  #@dim N: x.shape[0]; D: x.shape[1]
    #@requires x.dim() == 2
    return N, D


def unpack_prefix(x):
    B, T = x.shape[:2]  #@dim B: x.shape[0]; T: x.shape[1]
    #@requires x.dim() >= 2
    return B, T


def unpack_star(x):
    B, *rest = x.shape  #@dim B: x.shape[0]
    #@requires none
    y = torch.zeros(*rest)  #@shape y: [*x.shape[1:]]
    return y


def wrong(n):
    x = torch.zeros(n, 4)
    B, T, C = x.shape  #@shape-error
    return B, T, C


def concrete():
    a, b = torch.zeros(2, 5).shape  #@dim a: 2; b: 5
    return a, b


if __name__ == "__main__":
    unpack3(torch.randn(2, 3, 4))
    unpack_size(torch.randn(3, 4))
    unpack_prefix(torch.randn(2, 3, 4, 5))
    unpack_star(torch.randn(2, 3, 4))
    try:
        wrong(3)
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass
    concrete()

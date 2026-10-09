# einsum: equal letters broadcast size-1 dims (no requirement), ellipsis batch dims broadcast, a literal size clash is an error
import torch


def batched(a, b):
    B, I, J = a.shape
    B2, J2, K = b.shape
    z = torch.einsum("bij,bjk->bik", a, b)  #@shape z: [max(a.shape[0], b.shape[0]), a.shape[1], b.shape[2]]
    #@requires none
    return z


def ellipsis(a, b):
    I, J = a.shape
    z = torch.einsum("...ij,...jk->...ik", a, b)  #@shape z: [*b.shape[:-2], a.shape[0], b.shape[-1]]
    return z


def outer_trace(u, v, m):
    o = torch.einsum("i,j->ij", u, v)  #@shape o: [u.shape[0], v.shape[0]]
    t = torch.einsum("ii->", m)  #@shape t: []
    return o, t


def literal():
    a = torch.einsum("ij,jk->ik", torch.randn(2, 1), torch.randn(4, 5))  #@shape a: [2, 5]
    b = torch.einsum("bij,bjk->bik", torch.randn(1, 2, 3), torch.randn(4, 3, 5))  #@shape b: [4, 2, 5]
    return a, b


def clash():
    c = torch.einsum("ij,jk->ik", torch.randn(2, 3), torch.randn(4, 5))  #@shape-error
    return c


if __name__ == "__main__":
    z = batched(torch.randn(2, 3, 4), torch.randn(2, 4, 5))  #@shape z: [2, 3, 5]
    e = ellipsis(torch.randn(2, 3), torch.randn(6, 3, 5))  #@shape e: [6, 2, 5]
    outer_trace(torch.randn(3), torch.randn(4), torch.randn(5, 5))
    literal()
    try:
        clash()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

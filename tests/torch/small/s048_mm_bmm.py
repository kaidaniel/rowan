# mm needs two matrices, bmm two rank-3 tensors with equal batch: exact ranks and dim equalities required
import torch


def mm_unknown(a, b):
    c = a.mm(b)  #@shape c: [a.shape[0], b.shape[1]]
    #@requires a.dim() == 2; b.dim() == 2; a.shape[1] == b.shape[0]
    d = torch.mm(c, b.t())  #@shape d: [a.shape[0], a.shape[1]]
    #@requires none
    return c, d


def bmm_unknown(x, y):
    z = torch.bmm(x, y)  #@shape z: [x.shape[0], x.shape[1], y.shape[2]]
    #@requires x.dim() == 3; y.dim() == 3; x.shape[0] == y.shape[0]; x.shape[2] == y.shape[1]
    w = z.bmm(z.transpose(1, 2))  #@shape w: [x.shape[0], x.shape[1], x.shape[1]]
    #@requires none
    return z, w


def concrete():
    a = torch.bmm(torch.randn(4, 2, 3), torch.randn(4, 3, 5))  #@shape a: [4, 2, 5]
    b = torch.randn(2, 3).mm(torch.randn(3, 7))  #@shape b: [2, 7]
    return a, b


def bad_bmm_rank():
    return torch.bmm(torch.randn(2, 3), torch.randn(3, 4))  #@shape-error


def bad_mm_inner():
    return torch.mm(torch.randn(2, 3), torch.randn(4, 5))  #@shape-error


if __name__ == "__main__":
    mm_unknown(torch.randn(2, 3), torch.randn(3, 4))
    bmm_unknown(torch.randn(2, 3, 4), torch.randn(2, 4, 5))
    concrete()
    for bad in (bad_bmm_rank, bad_mm_inner):
        try:
            bad()
        except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
            pass

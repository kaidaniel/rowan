# einsum: the output subscripts pick dims from the operands (matmul, transpose, sums, attention, ellipsis)
import torch


def ein_basic(a, b):
    c = torch.einsum('ij,jk->ik', a, b)  #@shape c: [a.shape[0], b.shape[1]]
    t = torch.einsum('ij->ji', a)  #@shape t: [a.shape[1], a.shape[0]]
    s = torch.einsum('ij->', a)  #@shape s: []
    r = torch.einsum('ij->i', a)  #@shape r: [a.shape[0]]
    return c, t, s, r


def ein_attn(q):
    B, H, T, D = q.shape  #@requires q.dim() == 4
    att = torch.einsum('bhqd,bhkd->bhqk', q, q)  #@shape att: [q.shape[0], q.shape[1], q.shape[2], q.shape[2]]
    out = torch.einsum('bhqk,bhkd->bhqd', att, q)  #@shape out: [q.shape[0], q.shape[1], q.shape[2], q.shape[3]]
    return att, out


def ein_misc(x, u, v):
    y = torch.einsum('...ij->...ji', x)  #@shape y: [*x.shape[:-2], x.shape[-1], x.shape[-2]]
    o = torch.einsum('i,j->ij', u, v)  #@shape o: [u.shape[0], v.shape[0]]
    return y, o


def concrete():
    a = torch.einsum('bhqd,bhkd->bhqk', torch.randn(2, 4, 5, 8), torch.randn(2, 4, 7, 8))  #@shape a: [2, 4, 5, 7]
    b = torch.einsum('ii', torch.randn(3, 3))  #@shape b: []
    c = torch.einsum('bn,nm->bm', torch.randn(6, 2), torch.randn(2, 9))  #@shape c: [6, 9]
    return a, b, c


def bad_inner():
    return torch.einsum('ij,jk->ik', torch.randn(2, 3), torch.randn(4, 5))  #@shape-error


if __name__ == "__main__":
    ein_basic(torch.randn(2, 3), torch.randn(3, 4))
    ein_attn(torch.randn(2, 3, 5, 4))
    ein_misc(torch.randn(2, 3, 4), torch.randn(3), torch.randn(5))
    ein_misc(torch.randn(3, 4), torch.randn(1), torch.randn(2))
    concrete()
    try:
        bad_inner()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

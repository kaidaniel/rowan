# advanced indexing: boolean masks give an unknown count, LongTensor indices put idx.shape in front; gather, index_select
import torch


def masks(x):
    m = x > 0  #@shape m: [*x.shape]
    a = x[m]  #@shape a: [?]
    B, C = x.shape  #@requires x.dim() == 2
    keep = x[:, 0] > 0  #@shape keep: [x.shape[0]]
    r = x[keep]  #@shape r: [?, x.shape[1]]
    return a, r


def select(x, k):
    B, C = x.shape  #@requires x.dim() == 2
    idx = torch.arange(k)  #@shape idx: [k]
    a = x[idx]  #@shape a: [k, x.shape[1]]
    b = x[:, idx]  #@shape b: [x.shape[0], k]
    c = x.index_select(1, idx)  #@shape c: [x.shape[0], k]
    d = torch.index_select(x, 0, idx)  #@shape d: [k, x.shape[1]]
    gi = torch.zeros(B, 1, dtype=torch.long)
    g = x.gather(1, gi)  #@shape g: [x.shape[0], 1]
    g2 = torch.gather(x, -1, idx.expand(B, k))  #@shape g2: [x.shape[0], k]
    return a, b, c, d, g, g2


def lookup(table, n, m):
    V, D = table.shape  #@requires table.dim() == 2
    ids = torch.randint(0, V, (n, m))  #@shape ids: [n, m]
    e = table[ids]  #@shape e: [n, m, table.shape[1]]
    return e


def bad_select():
    x = torch.randn(3, 4)
    return x.index_select(2, torch.arange(2))  #@shape-error


if __name__ == "__main__":
    masks(torch.randn(4, 3))
    select(torch.randn(4, 6), 3)
    lookup(torch.randn(10, 5), 2, 3)
    try:
        bad_select()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

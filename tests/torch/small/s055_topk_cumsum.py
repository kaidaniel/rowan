# topk returns (values, indices) with k at dim; cumsum / cumprod / sort / cummax keep the shape
import torch


def top(x, k):
    v, i = x.topk(k)  #@shape v: [*x.shape[:-1], k]; i: [*x.shape[:-1], k]
    v2, i2 = torch.topk(x, k, dim=0)  #@shape v2: [k, *x.shape[1:]]; i2: [k, *x.shape[1:]]
    r = x.topk(3, dim=-1, largest=False).values  #@shape r: [*x.shape[:-1], 3]
    return v, i, v2, i2, r


def scans(x):
    B, T = x.shape  #@requires x.dim() == 2
    c = x.cumsum(dim=1)  #@shape c: [x.shape[0], x.shape[1]]
    p = torch.cumprod(x, 0)  #@shape p: [x.shape[0], x.shape[1]]
    s, idx = x.sort(dim=-1, descending=True)  #@shape s: [x.shape[0], x.shape[1]]; idx: [x.shape[0], x.shape[1]]
    cm = torch.cummax(x, dim=1).values  #@shape cm: [x.shape[0], x.shape[1]]
    #@requires none
    return c, p, s, idx, cm


def bad_k():
    return torch.randn(2, 3).topk(5)  #@shape-error


if __name__ == "__main__":
    top(torch.randn(4, 5), 2)
    top(torch.randn(3, 4, 6), 3)
    scans(torch.randn(3, 4))
    try:
        bad_k()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

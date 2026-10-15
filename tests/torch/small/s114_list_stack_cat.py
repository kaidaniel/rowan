# lists of tensors: stack / cat of literal lists, a list built in a loop of unknown length (one append in each of n rounds: n long; was `?`), a cat mismatch
import torch


def gather(x, n):
    B, T, C = x.shape
    #@requires x.dim() == 3
    outs = []
    for i in range(n):
        outs.append(x[:, i])
    s = torch.stack(outs, dim=1)  #@shape s: [x.shape[0], n, x.shape[2]]
    return s


def literal_lists(x):
    B, T, C = x.shape
    parts = [x, x, x]
    s3 = torch.stack(parts)  #@shape s3: [3, x.shape[0], x.shape[1], x.shape[2]]
    c = torch.cat(parts, dim=-1)  #@shape c: [x.shape[0], x.shape[1], 3*x.shape[2]]
    #@requires none
    c2 = torch.cat([x, x.mean(dim=1, keepdim=True)], dim=1)  #@shape c2: [x.shape[0], x.shape[1] + 1, x.shape[2]]
    #@requires none
    ends = torch.stack((x[:, 0], x[:, -1]), dim=-1)  #@shape ends: [x.shape[0], x.shape[2], 2]
    return s3, c, c2, ends


def cat_mismatch():
    a = torch.randn(2, 3)
    b = torch.randn(2, 4)
    c = torch.cat([a, b], dim=0)  #@shape-error
    return c


if __name__ == "__main__":
    s = gather(torch.randn(2, 5, 3), 4)  #@shape s: [2, 4, 3]
    s3, c, c2, ends = literal_lists(torch.randn(2, 5, 3))  #@shape s3: [3, 2, 5, 3]; c2: [2, 6, 3]
    try:
        cat_mismatch()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

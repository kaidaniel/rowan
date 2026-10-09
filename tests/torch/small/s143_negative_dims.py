# negative dims on a tensor of unknown rank (rank bounds recorded once, then facts), and the same ops after an exact rank turns them non-negative
import torch
import torch.nn.functional as F


def unknown_rank(x):
    p = F.softmax(x, -1)  #@shape p: [*x.shape]
    s = x.sum(-2)  #@shape s: [*x.shape[:-2], x.shape[-1]]
    #@requires x.dim() >= 2
    f = x.flatten(-2)  #@shape f: [*x.shape[:-2], x.shape[-2]*x.shape[-1]]
    #@requires none
    u = x.unsqueeze(-1)  #@shape u: [*x.shape, 1]
    v = x.unsqueeze(-2)  #@shape v: [*x.shape[:-1], 1, x.shape[-1]]
    m = x.mean(-3)  #@shape m: [*x.shape[:-3], x.shape[-2], x.shape[-1]]
    #@requires x.dim() >= 3
    t = x.transpose(-1, -3)  #@shape t: [*x.shape[:-3], x.shape[-1], x.shape[-2], x.shape[-3]]
    #@requires none
    return p, s, f, u, v, m, t


def known_rank(x):
    B, T, C = x.shape
    s = x.sum(-2)  #@shape s: [x.shape[0], x.shape[2]]
    v = x.unsqueeze(-2)  #@shape v: [x.shape[0], x.shape[1], 1, x.shape[2]]
    u = x.unsqueeze(-4)  #@shape u: [1, x.shape[0], x.shape[1], x.shape[2]]
    f = x.flatten(-3, -2)  #@shape f: [x.shape[0]*x.shape[1], x.shape[2]]
    c = torch.cat([x, x], -1)  #@shape c: [x.shape[0], x.shape[1], 2*x.shape[2]]
    k = torch.stack([x, x], -1)  #@shape k: [x.shape[0], x.shape[1], x.shape[2], 2]
    return s, v, u, f, c, k


if __name__ == "__main__":
    unknown_rank(torch.randn(2, 3, 4))
    unknown_rank(torch.randn(5, 2, 3, 4))
    known_rank(torch.randn(2, 3, 4))

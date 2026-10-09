# F.relu / F.gelu / F.dropout / F.silu keep the shape; F.normalize, F.softmax / F.log_softmax along a dim
import torch
import torch.nn.functional as F


def f_acts(x):
    a = F.relu(x)  #@shape a: [*x.shape]
    #@requires none
    b = F.gelu(a)  #@shape b: [*x.shape]
    c = F.dropout(b, p=0.2, training=True)  #@shape c: [*x.shape]
    d = F.silu(c)
    return d


def f_norm(x):
    y = F.normalize(x, dim=1)  #@shape y: [*x.shape]
    #@requires x.dim() >= 2
    z = F.normalize(x, p=2, dim=-1)  #@shape z: [*x.shape]
    #@requires none
    return y, z


def f_softmax_cols(x):
    N, C = x.shape
    p = F.softmax(x, dim=0)  #@shape p: [x.shape[0], x.shape[1]]
    lp = F.log_softmax(x, dim=1)  #@shape lp: [x.shape[0], x.shape[1]]
    s = p.sum(dim=1)  #@shape s: [x.shape[0]]
    return lp, s


if __name__ == "__main__":
    f_acts(torch.randn(3, 4))
    f_norm(torch.randn(2, 3, 4))
    f_softmax_cols(torch.randn(5, 6))

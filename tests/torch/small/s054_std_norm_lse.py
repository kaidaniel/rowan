# std / var / norm / logsumexp reduce like sum; softmax and log_softmax keep the shape
import torch
import torch.nn.functional as F


def stats(x):
    B, T, C = x.shape  #@requires x.dim() == 3
    s = x.std(dim=0)  #@shape s: [x.shape[1], x.shape[2]]
    v = x.var(dim=-1, keepdim=True)  #@shape v: [x.shape[0], x.shape[1], 1]
    n = x.norm(dim=-1, keepdim=True)  #@shape n: [x.shape[0], x.shape[1], 1]
    u = x / n  #@shape u: [x.shape[0], x.shape[1], x.shape[2]]
    #@requires none
    l = x.logsumexp(dim=1)  #@shape l: [x.shape[0], x.shape[2]]
    l2 = torch.logsumexp(x, -1, keepdim=True)  #@shape l2: [x.shape[0], x.shape[1], 1]
    nn_ = x.norm()  #@shape nn_: []
    sv = x.std()  #@shape sv: []
    return s, v, u, l, l2, nn_, sv


def soft(x):
    p = x.softmax(dim=-1)  #@shape p: [*x.shape]
    lp = F.log_softmax(x, dim=-1)  #@shape lp: [*x.shape]
    q = torch.softmax(x, -1)  #@shape q: [*x.shape]
    n = x.norm(dim=-1, keepdim=True)  #@shape n: [*x.shape[:-1], 1]
    u = x / n  #@shape u: [*x.shape]
    #@requires none
    return p, lp, q, u


def bad_dim():
    return torch.randn(2, 3).logsumexp(dim=2)  #@shape-error


if __name__ == "__main__":
    stats(torch.randn(2, 3, 4))
    soft(torch.randn(2, 3, 4))
    soft(torch.randn(5))
    try:
        bad_dim()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

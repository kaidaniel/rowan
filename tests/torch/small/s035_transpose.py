# transpose with positive and negative dims, on tensors of unknown and of known rank
import torch


def tr_unknown(x):
    y = x.transpose(1, 2)  #@shape y: [x.shape[0], x.shape[2], x.shape[1], *x.shape[3:]]
    #@requires x.dim() >= 3
    z = x.transpose(-2, -1)  #@shape z: [*x.shape[:-2], x.shape[-1], x.shape[-2]]
    return y, z


def tr_known(x):
    B, T, C = x.shape  #@requires x.dim() == 3
    a = x.transpose(0, 1)  #@shape a: [x.shape[1], x.shape[0], x.shape[2]]
    b = x.transpose(-1, 0)  #@shape b: [x.shape[2], x.shape[1], x.shape[0]]
    c = torch.transpose(x, 1, -1)  #@shape c: [x.shape[0], x.shape[2], x.shape[1]]
    #@requires none
    d = x.transpose(1, 1)  #@shape d: [x.shape[0], x.shape[1], x.shape[2]]
    e = c.transpose(-2, -1) + x  #@shape e: [x.shape[0], x.shape[1], x.shape[2]]
    #@requires none
    return a, b, c, d, e


def bad_dim():
    return torch.randn(2, 3).transpose(0, 2)  #@shape-error


if __name__ == "__main__":
    tr_unknown(torch.randn(2, 3, 4))
    tr_unknown(torch.randn(2, 3, 4, 5, 6))
    tr_known(torch.randn(2, 3, 4))
    try:
        bad_dim()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

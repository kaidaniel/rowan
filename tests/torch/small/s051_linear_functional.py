# F.linear(x, w, b): [*, in] -> [*, out] with w of shape (out, in)
import torch
import torch.nn.functional as F


def lin_known(x):
    w = torch.randn(8, 4)
    b = torch.randn(8)
    y = F.linear(x, w)  #@shape y: [*x.shape[:-1], 8]
    #@requires x.shape[-1] == 4
    z = F.linear(x, w, b)  #@shape z: [*x.shape[:-1], 8]
    #@requires none
    return y, z


def lin_params(x, w):
    out_f, in_f = w.shape  #@requires w.dim() == 2
    y = F.linear(x, w)  #@shape y: [*x.shape[:-1], w.shape[0]]
    #@requires x.shape[-1] == w.shape[1]
    z = F.linear(y, torch.randn(3, out_f), torch.zeros(3))  #@shape z: [*x.shape[:-1], 3]
    #@requires none
    return y, z


def concrete():
    a = F.linear(torch.randn(3, 4), torch.randn(5, 4))  #@shape a: [3, 5]
    b = F.linear(torch.randn(2, 6, 4), torch.randn(5, 4), torch.randn(5))  #@shape b: [2, 6, 5]
    return a, b


def bad_in():
    return F.linear(torch.randn(3, 4), torch.randn(5, 6))  #@shape-error


if __name__ == "__main__":
    lin_known(torch.randn(2, 3, 4))
    lin_known(torch.randn(4))
    lin_params(torch.randn(2, 4), torch.randn(6, 4))
    concrete()
    try:
        bad_in()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

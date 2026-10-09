# t(), .T on matrices, t() of a vector, and .mT on a batch of unknown rank
import torch


def mat(w):
    o, i = w.shape  #@requires w.dim() == 2
    a = w.t()  #@shape a: [w.shape[1], w.shape[0]]
    b = w.T  #@shape b: [w.shape[1], w.shape[0]]
    c = torch.t(w)  #@shape c: [w.shape[1], w.shape[0]]
    d = w.T @ w  #@shape d: [w.shape[1], w.shape[1]]
    #@requires none
    return a, b, c, d


def vec(n):
    v = torch.randn(n)
    a = v.t()  #@shape a: [n]
    return a


def batched(x):
    y = x.mT  #@shape y: [*x.shape[:-2], x.shape[-1], x.shape[-2]]
    return y


def gram(x):
    B, M, N = x.shape  #@requires x.dim() == 3
    s = x @ x.mT  #@shape s: [x.shape[0], x.shape[1], x.shape[1]]
    #@requires none
    return s


def bad_t():
    return torch.randn(2, 3, 4).t()  #@shape-error


if __name__ == "__main__":
    mat(torch.randn(3, 5))
    vec(4)
    batched(torch.randn(2, 3, 4))
    batched(torch.randn(6, 2, 3, 4))
    gram(torch.randn(2, 3, 4))
    try:
        bad_t()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

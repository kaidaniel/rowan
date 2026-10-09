# repeat: dims multiply by the repeat counts; extra counts add leading dims
import torch


def rep(x, n):
    B, T, C = x.shape  #@requires x.dim() == 3
    a = x.repeat(2, 1, 3)  #@shape a: [2*x.shape[0], x.shape[1], 3*x.shape[2]]
    b = x.repeat(n, 1, 1)  #@shape b: [n*x.shape[0], x.shape[1], x.shape[2]]
    c = x.repeat(4, 1, 1, 1)  #@shape c: [4, x.shape[0], x.shape[1], x.shape[2]]
    d = x.repeat((1, T, 1))  #@shape d: [x.shape[0], x.shape[1]*x.shape[1], x.shape[2]]
    #@requires none
    return a, b, c, d


def concrete():
    a = torch.zeros(3).repeat(2, 2)  #@shape a: [2, 6]
    b = torch.zeros(2, 3).repeat(1, 1)  #@shape b: [2, 3]
    return a, b


def bad_repeat():
    return torch.zeros(2, 3).repeat(2)  #@shape-error


if __name__ == "__main__":
    rep(torch.randn(2, 3, 4), 3)
    concrete()
    try:
        bad_repeat()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

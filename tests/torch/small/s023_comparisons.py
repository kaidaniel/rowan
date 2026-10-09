# comparisons and boolean ops give the broadcast shape: x > 0, x == y, &, |, ~, torch.eq, x.lt
import torch


def cmp(x):
    a = x > 0  #@shape a: [*x.shape]
    b = (x >= -1) & (x <= 1)  #@shape b: [*x.shape]
    c = ~a  #@shape c: [*x.shape]
    d = torch.eq(x, 0)  #@shape d: [*x.shape]
    e = x.lt(0.5) | a  #@shape e: [*x.shape]
    return a, b, c, d, e


def cmp_known(x):
    N, D = x.shape
    #@requires x.dim() == 2
    m = x > torch.zeros(D)  #@shape m: [x.shape[0], x.shape[1]]
    k = torch.arange(D) == torch.zeros(N, 1)  #@shape k: [x.shape[0], x.shape[1]]
    return m, k


def two(x, y):
    ne = x != y  #@shape ne: [*broadcast(x.shape, y.shape)]
    return ne


def concrete():
    p = torch.zeros(3, 1) == torch.zeros(4)  #@shape p: [3, 4]
    return p


def bad():
    q = torch.zeros(3) < torch.zeros(4)  #@shape-error
    return q


if __name__ == "__main__":
    cmp(torch.randn(2, 3))
    cmp_known(torch.randn(3, 5))
    two(torch.randn(2, 1), torch.randn(4))
    concrete()
    try:
        bad()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

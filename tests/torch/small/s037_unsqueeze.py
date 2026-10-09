# unsqueeze at positive / negative positions, on unknown and known ranks; broadcasting of the result
import torch


def unsq(x):
    a = x.unsqueeze(0)  #@shape a: [1, *x.shape]
    b = x.unsqueeze(-1)  #@shape b: [*x.shape, 1]
    c = x.unsqueeze(1)  #@shape c: [x.shape[0], 1, *x.shape[1:]]
    return a, b, c


def unsq_known(x):
    B, C = x.shape  #@requires x.dim() == 2
    a = x.unsqueeze(2)  #@shape a: [x.shape[0], x.shape[1], 1]
    b = x.unsqueeze(-2)  #@shape b: [x.shape[0], 1, x.shape[1]]
    c = torch.unsqueeze(x, 0).unsqueeze(0)  #@shape c: [1, 1, x.shape[0], x.shape[1]]
    m = x.unsqueeze(1) * x.unsqueeze(2)  #@shape m: [x.shape[0], x.shape[1], x.shape[1]]
    #@requires none
    return a, b, c, m


def bad_pos():
    x = torch.randn(2, 3)
    return x.unsqueeze(3)  #@shape-error


if __name__ == "__main__":
    unsq(torch.randn(4))
    unsq(torch.randn(2, 3, 5))
    unsq_known(torch.randn(3, 4))
    try:
        bad_pos()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

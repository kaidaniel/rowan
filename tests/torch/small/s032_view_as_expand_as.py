# view_as / reshape_as take the other tensor's shape; expand_as broadcasts size-1 dims up to it
import torch


def like(x, y):
    a = x.view_as(y)  #@shape a: [*y.shape]
    b = x.reshape_as(y)  #@shape b: [*y.shape]
    c = torch.ones(1).expand_as(y)  #@shape c: [*y.shape]
    return a, b, c


def bias_like(x):
    B, T, C = x.shape  #@requires x.dim() == 3
    b = torch.randn(1, 1, C)  #@shape b: [1, 1, x.shape[2]]
    e = b.expand_as(x)  #@shape e: [x.shape[0], x.shape[1], x.shape[2]]
    #@requires none
    m = torch.zeros(T, 1)
    f = m.expand_as(x)  #@shape f: [x.shape[0], x.shape[1], x.shape[2]]
    #@requires none
    g = x.reshape(B * T, C).view_as(x)  #@shape g: [x.shape[0], x.shape[1], x.shape[2]]
    return e, f, g


def concrete():
    a = torch.randn(3, 1).expand_as(torch.zeros(2, 3, 4))  #@shape a: [2, 3, 4]
    b = torch.arange(6).view_as(torch.zeros(2, 3))  #@shape b: [2, 3]
    return a, b


def bad_expand():
    return torch.randn(2, 3).expand_as(torch.randn(4, 3))  #@shape-error


if __name__ == "__main__":
    like(torch.randn(2, 6), torch.randn(3, 4))
    like(torch.randn(12), torch.randn(2, 2, 3))
    bias_like(torch.randn(2, 3, 4))
    concrete()
    try:
        bad_expand()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

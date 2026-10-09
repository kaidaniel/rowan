# tril / triu keep the shape and need at least 2 dims; torch.ones(T, T).tril() is a [T, T] causal mask
import torch


def masks(T):
    m = torch.ones(T, T).tril()  #@shape m: [T, T]
    u = torch.triu(torch.ones(T, T), diagonal=1)  #@shape u: [T, T]
    b = m == 0  #@shape b: [T, T]
    return m, u, b


def on_param(x):
    a = torch.tril(x)  #@shape a: [*x.shape]
    #@requires x.dim() >= 2
    b = x.triu(1)  #@shape b: [*x.shape]
    #@requires none
    return a, b


def rect():
    r = torch.ones(3, 5).tril(-1)  #@shape r: [3, 5]
    s = torch.zeros(2, 4, 3).triu()  #@shape s: [2, 4, 3]
    return r, s


def bad():
    v = torch.tril(torch.ones(4))  #@shape-error
    return v


if __name__ == "__main__":
    masks(4)
    on_param(torch.randn(2, 3, 3))
    rect()
    try:
        bad()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

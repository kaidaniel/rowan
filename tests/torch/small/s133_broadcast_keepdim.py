# broadcasting against keepdim reductions keeps the shape; symbolic dims that may be 1 give max and no error; literal mismatches are errors
import torch


def center(x):
    B, T, C = x.shape
    m = x.mean(dim=1, keepdim=True)  #@shape m: [x.shape[0], 1, x.shape[2]]
    y = x - m  #@shape y: [x.shape[0], x.shape[1], x.shape[2]]
    #@requires none
    z = x / x.sum(-1, keepdim=True)  #@shape z: [x.shape[0], x.shape[1], x.shape[2]]
    #@requires none
    return y, z


def softmax_manual(x):
    mx = x.amax(dim=-1, keepdim=True)  #@shape mx: [*x.shape[:-1], 1]
    e = (x - mx).exp()  #@shape e: [*x.shape]
    p = e / e.sum(-1, keepdim=True)  #@shape p: [*x.shape]
    return p


def maybe_one(x, y):
    N, M = x.shape
    P, Q = y.shape
    z = x + y  #@shape z: [max(x.shape[0], y.shape[0]), max(x.shape[1], y.shape[1])]
    #@requires none
    w = x * torch.ones(5)  #@shape w: [x.shape[0], max(x.shape[1], 5)]
    return z, w


def literal_ok():
    a = torch.randn(4, 1) + torch.randn(1, 5)  #@shape a: [4, 5]
    b = torch.randn(2, 3).unsqueeze(1) + torch.randn(4, 1)  #@shape b: [2, 4, 3]
    return a, b


def literal_bad():
    c = torch.randn(3, 4) + torch.randn(3, 5)  #@shape-error
    return c


if __name__ == "__main__":
    y, z = center(torch.randn(2, 3, 4))  #@shape y: [2, 3, 4]; z: [2, 3, 4]
    p = softmax_manual(torch.randn(2, 7))  #@shape p: [2, 7]
    z1, w1 = maybe_one(torch.randn(3, 1), torch.randn(3, 4))  #@shape z1: [3, 4]; w1: [3, 5]
    z2, w2 = maybe_one(torch.randn(1, 5), torch.randn(3, 5))  #@shape z2: [3, 5]; w2: [1, 5]
    literal_ok()
    try:
        literal_bad()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

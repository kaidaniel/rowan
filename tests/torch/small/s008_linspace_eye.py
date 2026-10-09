# torch.linspace(start, end, steps) and torch.eye(n) / eye(n, m): literal and symbolic sizes
import torch


def concrete():
    a = torch.linspace(0, 1, 5)  #@shape a: [5]
    b = torch.linspace(-1.0, 1.0, steps=11)  #@shape b: [11]
    c = torch.eye(3)  #@shape c: [3, 3]
    d = torch.eye(2, 4)  #@shape d: [2, 4]
    return a, b, c, d


def symbolic(n, m):
    a = torch.linspace(0, 1, n)  #@shape a: [n]
    b = torch.linspace(0.0, 2.0, steps=m)  #@shape b: [m]
    c = torch.eye(n)  #@shape c: [n, n]
    d = torch.eye(n, m)  #@shape d: [n, m]
    return a, b, c, d


def from_shape(x):
    N, D = x.shape
    #@requires x.dim() == 2
    i = torch.eye(D)  #@shape i: [x.shape[1], x.shape[1]]
    y = x + torch.eye(N, D)  #@shape y: [x.shape[0], x.shape[1]]
    return i, y


if __name__ == "__main__":
    concrete()
    symbolic(4, 6)
    from_shape(torch.randn(3, 5))

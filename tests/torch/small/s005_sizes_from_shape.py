# factories sized by another tensor's shape (rank unknown): x.shape, *x.shape, x.size(), slices of the shape
import torch


def build(x):
    a = torch.zeros(x.shape)  #@shape a: [*x.shape]
    #@requires none
    b = torch.ones(*x.shape)  #@shape b: [*x.shape]
    c = torch.randn(x.size())  #@shape c: [*x.shape]
    d = torch.empty(x.shape[1:])  #@shape d: [*x.shape[1:]]
    #@requires none
    e = torch.zeros(*x.shape[:-1], 5)  #@shape e: [*x.shape[:-1], 5]
    f = torch.full(x.shape, 0.5)  #@shape f: [*x.shape]
    g = torch.rand(3, *x.shape)  #@shape g: [3, *x.shape]
    return a, b, c, d, e, f, g


def concrete():
    x = torch.zeros(2, 3, 4)
    a = torch.ones(x.shape[1:])  #@shape a: [3, 4]
    b = torch.zeros(*x.shape[:-1], 7)  #@shape b: [2, 3, 7]
    return a, b


if __name__ == "__main__":
    build(torch.randn(2, 3, 4))
    build(torch.randn(6))
    concrete()

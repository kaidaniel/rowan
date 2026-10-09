# sizes as a tuple, a list, size=, a tuple held in a local, the empty tuple; torch.full(size, value)
import torch


def make(n, m):
    a = torch.zeros((n, m))  #@shape a: [n, m]
    b = torch.ones([m, 2])  #@shape b: [m, 2]
    c = torch.randn(size=(n, 3))  #@shape c: [n, 3]
    d = torch.full((n, m), 7.0)  #@shape d: [n, m]
    e = torch.empty(size=[2, n])  #@shape e: [2, n]
    shp = (m, n, 1)
    f = torch.rand(shp)  #@shape f: [m, n, 1]
    return a, b, c, d, e, f


def concrete():
    a = torch.zeros(())  #@shape a: []
    b = torch.full((2, 3), -1)  #@shape b: [2, 3]
    c = torch.ones(size=(5,))  #@shape c: [5]
    d = torch.zeros((0, 4))  #@shape d: [0, 4]
    return a, b, c, d


if __name__ == "__main__":
    make(3, 4)
    concrete()

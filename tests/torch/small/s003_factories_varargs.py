# zeros/ones/empty/rand/randn with sizes as varargs: literal sizes and sizes from parameters
import torch


def make(n, m):
    a = torch.zeros(n, m)  #@shape a: [n, m]
    b = torch.ones(n)  #@shape b: [n]
    c = torch.empty(2, n, 3)  #@shape c: [2, n, 3]
    d = torch.rand(m, m)  #@shape d: [m, m]
    e = torch.randn(n, 1, m, dtype=torch.float64)  #@shape e: [n, 1, m]
    #@requires none
    return a, b, c, d, e


def concrete():
    a = torch.zeros(4, 5)  #@shape a: [4, 5]
    b = torch.randn(7)  #@shape b: [7]
    c = torch.ones(1, 2, 3, 4)  #@shape c: [1, 2, 3, 4]
    d = torch.empty(0)  #@shape d: [0]
    return a, b, c, d


if __name__ == "__main__":
    make(3, 4)
    make(1, 6)
    concrete()

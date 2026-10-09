# x.new_zeros / new_ones / new_full / new_empty / new_tensor: the size comes from the arguments, not from x
import torch


def make(x, n):
    a = x.new_zeros(2, 3)  #@shape a: [2, 3]
    b = x.new_ones((n, 4))  #@shape b: [n, 4]
    c = x.new_full((n,), 1.5)  #@shape c: [n]
    d = x.new_empty(x.shape)  #@shape d: [*x.shape]
    e = x.new_zeros(x.shape[:-1])  #@shape e: [*x.shape[:-1]]
    f = x.new_ones(())  #@shape f: []
    g = x.new_tensor([[1, 2, 3]])  #@shape g: [1, 3]
    return a, b, c, d, e, f, g


def known(x):
    B, T, C = x.shape
    #@requires x.dim() == 3
    h = x.new_zeros(B, C)  #@shape h: [x.shape[0], x.shape[2]]
    return h


if __name__ == "__main__":
    make(torch.randn(5, 6), 3)
    known(torch.randn(2, 3, 4))

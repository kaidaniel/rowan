# slices of x.shape: rests like *x.shape[1:] while the rank is unknown, explicit dims once it is known
import torch


def unknown_rank(x):
    a = x.shape[1:]  #@shape a: [*x.shape[1:]]
    b = x.shape[:-1]  #@shape b: [*x.shape[:-1]]
    c = torch.zeros(*x.shape[:-1], 8)  #@shape c: [*x.shape[:-1], 8]
    d = torch.ones(4, *x.shape[1:])  #@shape d: [4, *x.shape[1:]]
    #@requires none
    return a, b, c, d


def known_rank(x):
    B, T, C = x.shape
    #@requires x.dim() == 3
    a = x.shape[1:]  #@shape a: [x.shape[1], x.shape[2]]
    b = x.shape[-2:]  #@shape b: [x.shape[1], x.shape[2]]
    c = torch.zeros(*x.shape[:-1], 2 * C)  #@shape c: [x.shape[0], x.shape[1], 2*x.shape[2]]
    e = x.shape[3:]  #@shape e: []
    return a, b, c, e


if __name__ == "__main__":
    unknown_rank(torch.randn(2, 3, 4))
    unknown_rank(torch.randn(5))
    known_rank(torch.randn(2, 3, 4))

# shape queries on a tensor of unknown rank: shape, size(), size(i), size(-1), dim(), ndim; dims >= 2 are requirements
import torch


def queries(x):
    s = x.shape  #@shape s: [*x.shape]
    t = x.size()  #@shape t: [*x.shape]
    r = x.dim()  #@dim r: x.dim()
    r2 = x.ndim  #@dim r2: x.dim()
    last = x.size(-1)  #@dim last: x.shape[-1]
    #@requires none
    first = x.size(0)  #@dim first: x.shape[0]
    #@requires none
    second = x.shape[1]  #@dim second: x.shape[1]
    #@requires x.dim() >= 2
    third = x.size(2)  #@dim third: x.shape[2]
    #@requires x.dim() >= 3
    pen = x.size(-2)  #@dim pen: x.shape[-2]
    #@requires none
    return s, t, r, r2, last, first, second, third, pen


def known():
    x = torch.zeros(2, 3, 4)
    r = x.dim()  #@dim r: 3
    n = x.ndim  #@dim n: 3
    a = x.size(-1)  #@dim a: 4
    b = x.shape[-2]  #@dim b: 3
    return r, n, a, b


def bad():
    x = torch.zeros(2, 3)
    k = x.size(2)  #@shape-error
    return k


if __name__ == "__main__":
    queries(torch.randn(2, 3, 4))
    queries(torch.randn(5, 4, 3, 2))
    known()
    try:
        bad()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

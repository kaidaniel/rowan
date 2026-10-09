# functools.partial(F.softmax, dim=-1), partial(torch.sum, dim=1), lambdas and map over tensors
from functools import partial

import torch
import torch.nn.functional as F

softmax_last = partial(F.softmax, dim=-1)
sum_dim1 = partial(torch.sum, dim=1)
square = lambda t: t * t


def probs(logits):
    p = softmax_last(logits)  #@shape p: [*logits.shape]
    #@requires none
    s = sum_dim1(logits)  #@shape s: [logits.shape[0], *logits.shape[2:]]
    #@requires logits.dim() >= 2
    q = square(logits)  #@shape q: [*logits.shape]
    return p, s, q


def means(a, b):
    B, T = a.shape
    C, D = b.shape
    ms = list(map(lambda t: t.mean(0), [a, b]))
    m0 = ms[0]  #@shape m0: [a.shape[1]]
    m1 = ms[1]  #@shape m1: [b.shape[1]]
    return m0, m1


if __name__ == "__main__":
    p, s, q = probs(torch.randn(2, 3, 4))  #@shape p: [2, 3, 4]; s: [2, 4]; q: [2, 3, 4]
    m0, m1 = means(torch.randn(5, 6), torch.randn(2, 7))  #@shape m0: [6]; m1: [7]

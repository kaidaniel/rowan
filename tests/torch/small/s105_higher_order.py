# higher-order functions: apply(f, x) with a layer or a lambda, compose(f, g), twice(f, x)
import torch
import torch.nn as nn


def apply(f, x):
    r = f(x)  #@shape r: ?
    return r


def compose(f, g):
    return lambda x: f(g(x))


def twice(f, x):
    return f(f(x))


def run(x):
    B, T, C = x.shape
    #@requires x.dim() == 3
    lin = nn.Linear(C, 2 * C)
    y = apply(lin, x)  #@shape y: [x.shape[0], x.shape[1], 2*x.shape[2]]
    swap = lambda t: t.transpose(1, 2)
    z = apply(swap, x)  #@shape z: [x.shape[0], x.shape[2], x.shape[1]]
    h = compose(torch.relu, nn.Linear(2 * C, C))(y)  #@shape h: [x.shape[0], x.shape[1], x.shape[2]]
    s = twice(torch.tanh, x)  #@shape s: [x.shape[0], x.shape[1], x.shape[2]]
    return y, z, h, s


if __name__ == "__main__":
    run(torch.randn(2, 3, 4))
    r = apply(nn.Linear(4, 9), torch.randn(5, 4))  #@shape r: [5, 9]
    first_sum = lambda u: u.sum(0)
    t = twice(first_sum, torch.randn(3, 4, 5))  #@shape t: [5]

# nn.Linear whose sizes come from function parameters: symbolic dims, requirement on the last dim
import torch
from torch import nn


def make_and_apply(x, d_in, d_out):
    lin = nn.Linear(d_in, d_out)
    y = lin(x)  #@shape y: [*x.shape[:-1], d_out]
    #@requires x.shape[-1] == d_in
    w = lin.weight  #@shape w: [d_out, d_in]
    b = lin.bias
    n = lin.out_features  #@dim n: d_out
    return y, w, b, n


def no_bias(x, d_in, d_out):
    lin = nn.Linear(d_in, d_out, bias=False)
    b = lin.bias  #@shape b: none
    y = lin(x)
    return y, b


def two_layers(x, d_in, d_hidden, d_out):
    l1 = nn.Linear(d_in, d_hidden)
    l2 = nn.Linear(d_hidden, d_out)
    h = l1(x)  #@shape h: [*x.shape[:-1], d_hidden]
    #@requires x.shape[-1] == d_in
    y = l2(h)  #@shape y: [*x.shape[:-1], d_out]
    #@requires none
    return y


def applied_twice(x, d_in, d_out):
    lin = nn.Linear(d_in, d_out)
    y = lin(lin(x))
    #@requires x.shape[-1] == d_in; d_out == d_in
    return y


if __name__ == "__main__":
    make_and_apply(torch.randn(2, 5, 6), 6, 3)
    no_bias(torch.randn(6), 6, 4)
    two_layers(torch.randn(3, 8), 8, 16, 2)
    applied_twice(torch.randn(4, 8), 8, 8)

# closures returned by factories, capturing a layer and its dims or a head count
import torch
import torch.nn as nn


def make_head(d_in, n_out):
    lin = nn.Linear(d_in, n_out)

    def head(x):
        _ = (d_in, n_out)
        y = lin(x)  #@shape y: [*x.shape[:-1], n_out]
        #@requires x.shape[-1] == d_in
        return y

    return head


def make_splitter(h):
    def split(x):
        B, T, C = x.shape
        #@requires x.dim() == 3
        z = x.view(B, T, h, C // h)  #@shape z: [x.shape[0], x.shape[1], h, x.shape[2]//h]
        return z.transpose(1, 2)

    return split


def use(x):
    B, T, C = x.shape
    head = make_head(C, 10)
    y = head(x)  #@shape y: [x.shape[0], x.shape[1], 10]
    split = make_splitter(2)
    z = split(x)  #@shape z: [x.shape[0], 2, x.shape[1], x.shape[2]//2]
    return y, z


if __name__ == "__main__":
    y, z = use(torch.randn(3, 5, 8))  #@shape y: [3, 5, 10]; z: [3, 2, 5, 4]
    w = make_head(4, 6)(torch.randn(4))  #@shape w: [6]

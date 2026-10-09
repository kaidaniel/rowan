# a ModuleList of layers with different widths iterated in a loop: the shape after the loop is the last layer's, and a bad second layer is a definite error
import torch
import torch.nn as nn


class Funnel(nn.Module):
    def __init__(self, d):
        super().__init__()
        self.d = d
        self.layers = nn.ModuleList([nn.Linear(d, 2 * d), nn.ReLU(), nn.Linear(2 * d, d // 2)])

    def forward(self, x):
        h = x
        for layer in self.layers:
            h = layer(h)
            #@requires x.shape[-1] == self.d
        out = h  #@shape out: [*x.shape[:-1], self.d // 2]
        return out

    def last(self, z):
        y = self.layers[-1](z)  #@shape y: [*z.shape[:-1], self.d // 2]
        #@requires z.shape[-1] == 2*self.d
        return y


def wrong_order():
    layers = nn.ModuleList([nn.Linear(4, 8), nn.Linear(4, 2)])
    h = torch.randn(3, 4)
    for layer in layers:
        h = layer(h)  #@shape-error
    return h


if __name__ == "__main__":
    f = Funnel(8)
    y = f(torch.randn(3, 8))  #@shape y: [3, 4]
    z = f(torch.randn(2, 5, 8))  #@shape z: [2, 5, 4]
    w = f.last(torch.randn(6, 16))  #@shape w: [6, 4]
    try:
        wrong_order()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

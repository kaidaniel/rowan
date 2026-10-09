# nn.Sequential from a comprehension of unknown length (possibly empty) and from an OrderedDict: names, indices, len
from collections import OrderedDict

import torch
import torch.nn as nn


class Block(nn.Module):
    def __init__(self, d):
        super().__init__()
        self.fc = nn.Linear(d, d)

    def forward(self, x):
        return x + torch.relu(self.fc(x))


class Net(nn.Module):
    def __init__(self, d, n, n_out):
        super().__init__()
        self.d = d
        self.n_out = n_out
        self.blocks = nn.Sequential(*[Block(d) for _ in range(n)])
        self.head = nn.Sequential(OrderedDict([
            ("proj", nn.Linear(d, 2 * d)),
            ("act", nn.GELU()),
            ("out", nn.Linear(2 * d, n_out)),
        ]))

    def forward(self, x):
        assert x.shape[-1] == self.d
        h = self.blocks(x)  #@shape h: [*x.shape[:-1], self.d]
        #@requires none
        p = self.head.proj(h)  #@shape p: [*x.shape[:-1], 2*self.d]
        #@requires none
        y = self.head(h)  #@shape y: [*x.shape[:-1], self.n_out]
        #@requires none
        k = len(self.head)  #@dim k: 3
        q = self.head[2](p)  #@shape q: [*x.shape[:-1], self.n_out]
        #@requires none
        return y, q


if __name__ == "__main__":
    net = Net(8, 3, 5)
    y, q = net(torch.randn(2, 4, 8))  #@shape y: [2, 4, 5]; q: [2, 4, 5]
    empty = Net(8, 0, 5)
    y2, q2 = empty(torch.randn(6, 8))  #@shape y2: [6, 5]; q2: [6, 5]

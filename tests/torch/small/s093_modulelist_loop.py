# ModuleList of shape-preserving layers iterated in forward; the assert gives the last dim
import torch
import torch.nn as nn


class Stack(nn.Module):
    def __init__(self, d, n_layers):
        super().__init__()
        self.d = d
        self.layers = nn.ModuleList([nn.Linear(d, d) for _ in range(n_layers)])
        self.norm = nn.LayerNorm(d)

    def forward(self, x):
        assert x.shape[-1] == self.d
        #@requires none
        h = x
        for layer in self.layers:
            h = torch.relu(layer(h))  #@shape h: [*x.shape[:-1], self.d]
            #@requires none
        out = self.norm(h)  #@shape out: [*x.shape[:-1], self.d]
        #@requires none
        return out

    def first(self, x):
        y = self.layers[0](x)  #@shape y: [*x.shape[:-1], self.d]
        #@requires x.shape[-1] == self.d
        return y


if __name__ == "__main__":
    s = Stack(8, 3)
    y = s(torch.randn(2, 8))  #@shape y: [2, 8]
    z = s(torch.randn(4, 5, 8))  #@shape z: [4, 5, 8]
    f = s.first(torch.randn(6, 8))  #@shape f: [6, 8]

# tensors built in list comprehensions / generator expressions, then stacked or concatenated (multi-head)
import torch
import torch.nn as nn


class Head(nn.Module):
    def __init__(self, d, hs):
        super().__init__()
        self.proj = nn.Linear(d, hs)

    def forward(self, x):
        return self.proj(x)


class MultiHead(nn.Module):
    def __init__(self, d, n, hs):
        super().__init__()
        self.d = d
        self.heads = nn.ModuleList([Head(d, hs) for _ in range(n)])
        self.out = nn.Linear(n * hs, d)

    def forward(self, x):
        outs = [h(x) for h in self.heads]
        y = torch.cat(outs, dim=-1)  #@shape y: [*x.shape[:-1], _]
        z = self.out(y)  #@shape z: [*x.shape[:-1], self.d]
        return z


def comps(x):
    B, T, C = x.shape
    outs = [x[:, i] for i in range(3)]
    s = torch.stack(outs, dim=1)  #@shape s: [x.shape[0], 3, x.shape[2]]
    scaled = tuple(x * k for k in range(4))
    g = torch.cat(scaled, dim=0)  #@shape g: [4*x.shape[0], x.shape[1], x.shape[2]]
    layers = [nn.Linear(C, 5) for _ in range(2)]
    st = torch.stack([lin(x) for lin in layers])  #@shape st: [2, x.shape[0], x.shape[1], 5]
    #@requires none
    terms = (x * k for k in range(3))
    tot = sum(terms)  #@shape tot: [x.shape[0], x.shape[1], x.shape[2]]
    filt = [x[:, i] for i in range(4) if i % 2 == 0]
    f = torch.stack(filt, dim=0)  #@shape f: [2, x.shape[0], x.shape[2]]
    return s, g, st, tot, f


if __name__ == "__main__":
    mh = MultiHead(8, 3, 4)
    z = mh(torch.randn(2, 5, 8))  #@shape z: [2, 5, 8]
    s, g, st, tot, f = comps(torch.randn(2, 5, 3))  #@shape s: [2, 3, 3]; g: [8, 5, 3]; st: [2, 2, 5, 5]; f: [2, 2, 3]

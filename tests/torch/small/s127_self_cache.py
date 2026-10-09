# tensors stored on self during forward (a cache, a growing KV buffer) and read later in another method
import torch
import torch.nn as nn


class Cached(nn.Module):
    def __init__(self, d):
        super().__init__()
        self.d = d
        self.proj = nn.Linear(d, 2 * d)
        self.last = None

    def forward(self, x):
        h = self.proj(x)  #@shape h: [*x.shape[:-1], 2*self.d]
        #@requires x.shape[-1] == self.d
        self.last = h
        return h

    def pooled(self):
        if self.last is None:
            return None
        s = self.last.mean(dim=-1)  #@shape@L44 s: [2, 3]
        return s


class KV(nn.Module):
    def __init__(self):
        super().__init__()
        self.k = None

    def step(self, k):
        if self.k is None:
            self.k = k
        else:
            self.k = torch.cat([self.k, k], dim=1)
        c = self.k  #@shape@L47 c: [2, 1, 4]
        #@shape@L48 c: [2, 2, 4]
        return c


if __name__ == "__main__":
    m = Cached(4)
    h = m(torch.randn(2, 3, 4))  #@shape h: [2, 3, 8]
    s = m.pooled()  #@shape s: [2, 3]
    m2 = Cached(4)
    kv = KV()
    c1 = kv.step(torch.randn(2, 1, 4))  #@shape c1: [2, 1, 4]
    c2 = kv.step(torch.randn(2, 1, 4))  #@shape c2: [2, 2, 4]
    c3 = kv.step(torch.randn(2, 1, 4))  #@shape c3: [2, 3, 4]
    p0 = m2.pooled()  #@shape p0: none

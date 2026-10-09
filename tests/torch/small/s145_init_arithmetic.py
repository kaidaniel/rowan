# __init__ computes widths with arithmetic and a conditional: forward dims follow the attribute that holds the join, decided per branch in concrete calls
import torch
import torch.nn as nn


class FFN(nn.Module):
    def __init__(self, d, expand):
        super().__init__()
        self.d = d
        hidden = d * 4 if expand else d
        self.hidden = hidden
        self.up = nn.Linear(d, hidden)
        self.down = nn.Linear(hidden, d)

    def forward(self, x):
        h = self.up(x)  #@shape h: [*x.shape[:-1], self.hidden]
        #@shape@L45 h: [2, 32]
        #@shape@L46 h: [2, 8]
        #@requires x.shape[-1] == self.d
        y = self.down(h)  #@shape y: [*x.shape[:-1], self.d]
        #@requires none
        return y


class Heads(nn.Module):
    def __init__(self, d, n_heads):
        super().__init__()
        self.n_heads = n_heads
        self.head_dim = d // n_heads
        self.inner = self.n_heads * self.head_dim
        self.qkv = nn.Linear(d, 3 * self.inner)

    def forward(self, x):
        B, T, C = x.shape
        qkv = self.qkv(x)  #@shape qkv: [x.shape[0], x.shape[1], 3*self.inner]
        #@requires x.shape[2] == d
        q, k, v = qkv.chunk(3, dim=-1)  #@shape q: [x.shape[0], x.shape[1], self.inner]
        qh = q.view(B, T, self.n_heads, self.head_dim)  #@shape qh: [x.shape[0], x.shape[1], self.n_heads, self.head_dim]
        return qh


if __name__ == "__main__":
    big = FFN(8, True)
    small = FFN(8, False)
    a = big(torch.randn(2, 8))  #@shape a: [2, 8]
    b = small(torch.randn(2, 8))  #@shape b: [2, 8]
    hd = Heads(12, 4)
    qh = hd(torch.randn(2, 5, 12))  #@shape qh: [2, 5, 4, 3]

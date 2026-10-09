# nn.Parameter attributes: x @ self.w and a bias, dims from __init__ (self.d_out alias, d_in none)
import torch
import torch.nn as nn


class Proj(nn.Module):
    def __init__(self, d_in, d_out):
        super().__init__()
        self.w = nn.Parameter(torch.randn(d_in, d_out))
        self.b = nn.Parameter(torch.zeros(d_out))
        self.d_out = d_out

    def forward(self, x):
        B, T, C = x.shape
        #@requires x.dim() == 3
        w = self.w  #@shape w: [d_in, self.d_out]
        h = x @ self.w  #@shape h: [x.shape[0], x.shape[1], self.d_out]
        #@requires x.shape[2] == d_in
        y = h + self.b  #@shape y: [x.shape[0], x.shape[1], self.d_out]
        #@requires none
        return y

    def transposed(self):
        wt = self.w.t()  #@shape wt: [self.d_out, d_in]
        return wt


if __name__ == "__main__":
    p = Proj(6, 3)
    y = p(torch.randn(2, 4, 6))  #@shape y: [2, 4, 3]
    wt = p.transposed()  #@shape wt: [3, 6]

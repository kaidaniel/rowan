# forward calling self.split_heads / self.merge_heads; d_head = d_model // n_heads as self attributes
import torch
import torch.nn as nn
import torch.nn.functional as F


class MHA(nn.Module):
    def __init__(self, d_model, n_heads):
        super().__init__()
        self.d_model = d_model
        self.n_heads = n_heads
        self.d_head = d_model // n_heads
        self.qkv = nn.Linear(d_model, 3 * d_model)
        self.out = nn.Linear(d_model, d_model)

    def split_heads(self, t):
        B, T, C = t.shape
        #@requires t.dim() == 3
        h = t.view(B, T, self.n_heads, self.d_head)  #@shape h: [t.shape[0], t.shape[1], self.n_heads, self.d_head]
        #@requires none
        h = h.transpose(1, 2)  #@shape h: [t.shape[0], self.n_heads, t.shape[1], self.d_head]
        return h

    def merge_heads(self, h):
        B, H, T, D = h.shape
        #@requires h.dim() == 4
        y = h.transpose(1, 2).contiguous().view(B, T, H * D)  #@shape y: [h.shape[0], h.shape[2], h.shape[1]*h.shape[3]]
        return y

    def forward(self, x):
        B, T, C = x.shape
        q, k, v = self.qkv(x).chunk(3, dim=-1)  #@shape q: [x.shape[0], x.shape[1], self.d_model]
        #@requires x.shape[2] == self.d_model
        q, k, v = self.split_heads(q), self.split_heads(k), self.split_heads(v)
        #@shape k: [x.shape[0], self.n_heads, x.shape[1], self.d_head]
        y = F.scaled_dot_product_attention(q, k, v)  #@shape y: [x.shape[0], self.n_heads, x.shape[1], self.d_head]
        #@requires none
        y = self.merge_heads(y)  #@shape y: [x.shape[0], x.shape[1], self.n_heads*self.d_head]
        out = self.out(y)  #@shape out: [x.shape[0], x.shape[1], self.d_model]
        #@requires self.n_heads*self.d_head == self.d_model
        return out


if __name__ == "__main__":
    mha = MHA(16, 4)
    out = mha(torch.randn(2, 5, 16))  #@shape out: [2, 5, 16]
    h = mha.split_heads(torch.randn(3, 7, 16))  #@shape h: [3, 4, 7, 4]

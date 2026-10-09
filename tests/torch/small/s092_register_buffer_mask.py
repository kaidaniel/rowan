# register_buffer("mask", tril(ones(n, n))) sliced with T inside forward; split into q, k, v
import math

import torch
import torch.nn as nn


class CausalSelfAttention(nn.Module):
    def __init__(self, n_embd, block_size):
        super().__init__()
        self.n_embd = n_embd
        self.c_attn = nn.Linear(n_embd, 3 * n_embd)
        self.register_buffer("mask", torch.tril(torch.ones(block_size, block_size)))

    def forward(self, x):
        B, T, C = x.shape
        #@requires x.dim() == 3
        mask = self.mask  #@shape mask: [block_size, block_size]
        qkv = self.c_attn(x)  #@shape qkv: [x.shape[0], x.shape[1], 3*self.n_embd]
        #@requires x.shape[2] == self.n_embd
        q, k, v = qkv.split(self.n_embd, dim=2)  #@shape q: [x.shape[0], x.shape[1], self.n_embd]; v: [x.shape[0], x.shape[1], self.n_embd]
        att = q @ k.transpose(-2, -1) * (1.0 / math.sqrt(k.size(-1)))  #@shape att: [x.shape[0], x.shape[1], x.shape[1]]
        #@requires none
        m = self.mask[:T, :T]  #@shape m: [min(block_size, x.shape[1]), min(block_size, x.shape[1])]
        #@requires none
        att = att.masked_fill(m == 0, float("-inf"))
        y = att.softmax(dim=-1) @ v  #@shape y: [x.shape[0], _, self.n_embd]
        return y


if __name__ == "__main__":
    attn = CausalSelfAttention(12, 16)
    y = attn(torch.randn(2, 10, 12))  #@shape y: [2, 10, 12]
    m = attn.mask[:4, :4]  #@shape m: [4, 4]

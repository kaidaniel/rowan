# A dim that is one of two values (a join: `head_dim` is `dim // num_heads` or the
# `attn_head_dim` given) learnt equal to a shape dim must not be substituted into itself:
# rowan's dim normaliser recursed without end (a stack overflow on timm's beit Attention, in
# the run of `forward` with open parameters, which nothing here calls). Expectations written
# from the program text.
import torch
import torch.nn as nn


class Attention(nn.Module):
    def __init__(self, dim, num_heads, attn_head_dim=None):
        super().__init__()
        head_dim = dim // num_heads
        if attn_head_dim is not None:
            head_dim = attn_head_dim
        self.qkv = nn.Linear(dim, dim)
        self.proj = nn.Linear(head_dim * num_heads, dim)

    def forward(self, x):
        if self.q_bias is None:
            qkv = self.qkv(x)
        else:
            qkv += 1
        return self.proj(x)


t = torch.zeros(2, 3)          #@shape t: [2, 3]

# nn.MultiheadAttention on a parameter: self-attention on unknown rank, need_weights=False gives None, cross-attention
import torch
from torch import nn


def self_attn(x):
    attn = nn.MultiheadAttention(32, 4, batch_first=True)
    out, w = attn(x, x, x)  #@shape out: [*x.shape[:-1], 32]; w: [*, x.shape[-2], x.shape[-2]]
    #@requires x.dim() in (2, 3); x.shape[-1] == 32
    return out, w


def no_weights(x):
    T, B, C = x.shape
    attn = nn.MultiheadAttention(C, 1)
    out, w = attn(x, x, x, need_weights=False)  #@shape out: [x.shape[0], x.shape[1], x.shape[2]]; w: none
    #@requires none
    return out


def cross(x):
    B, T, C = x.shape
    mem = torch.randn(B, 7, C)
    attn = nn.MultiheadAttention(C, 1, batch_first=True)
    out, w = attn(x, mem, mem)  #@shape out: [x.shape[0], x.shape[1], x.shape[2]]; w: [x.shape[0], x.shape[1], 7]
    return out, w


if __name__ == "__main__":
    self_attn(torch.randn(2, 5, 32))
    self_attn(torch.randn(5, 32))
    no_weights(torch.randn(4, 2, 6))
    cross(torch.randn(2, 3, 8))

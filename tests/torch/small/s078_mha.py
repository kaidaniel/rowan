# nn.MultiheadAttention, seq-first and batch_first, key length S different from query length L
import torch
from torch import nn


def mha_seq_first(L, S, N):
    attn = nn.MultiheadAttention(16, 4)
    q = torch.randn(L, N, 16)
    k = torch.randn(S, N, 16)
    v = torch.randn(S, N, 16)
    out, w = attn(q, k, v)  #@shape out: [L, N, 16]; w: [N, L, S]
    #@requires none
    return out, w


def mha_batch_first(L, S, N):
    attn = nn.MultiheadAttention(24, 2, batch_first=True)
    q = torch.randn(N, L, 24)
    kv = torch.randn(N, S, 24)
    out, w = attn(q, kv, kv)  #@shape out: [N, L, 24]; w: [N, L, S]
    _, per_head = attn(q, kv, kv, average_attn_weights=False)  #@shape per_head: [N, 2, L, S]
    return out, w, per_head


def bad_embed():
    attn = nn.MultiheadAttention(16, 4)
    q = torch.randn(3, 2, 8)
    out, w = attn(q, q, q)  #@shape-error
    return out


if __name__ == "__main__":
    mha_seq_first(3, 5, 2)
    mha_batch_first(4, 6, 3)
    try:
        bad_embed()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

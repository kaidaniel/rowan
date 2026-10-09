# a plain attention(q, k, v, mask=None) helper on parameters: rank facts from asserts, broadcast batch dims
import math

import torch
import torch.nn.functional as F


def attention(q, k, v, mask=None):
    assert q.dim() == 4
    assert k.dim() == 4
    assert v.dim() == 4
    d = q.size(-1)  #@dim d: q.shape[3]
    kt = k.transpose(-2, -1)  #@shape kt: [k.shape[0], k.shape[1], k.shape[3], k.shape[2]]
    #@requires none
    scores = q @ kt / math.sqrt(d)  #@shape scores: [max(q.shape[0], k.shape[0]), max(q.shape[1], k.shape[1]), q.shape[2], k.shape[2]]
    #@requires q.shape[3] == k.shape[3]
    if mask is not None:
        scores.masked_fill_(mask == 0, float("-inf"))
    p = F.softmax(scores, dim=-1)  #@shape p: [max(q.shape[0], k.shape[0]), max(q.shape[1], k.shape[1]), q.shape[2], k.shape[2]]
    out = p @ v  #@shape out: [_, _, q.shape[2], v.shape[3]]
    #@requires k.shape[2] == v.shape[2]
    return out


def attention_unknown_rank(q, k):
    kt = k.transpose(-2, -1)  #@shape kt: [*k.shape[:-2], k.shape[-1], k.shape[-2]]
    #@requires k.dim() >= 2
    s = q @ kt  #@shape s: [*broadcast(q.shape[:-2], k.shape[:-2]), q.shape[-2], k.shape[-2]] if q.dim() >= 2
    #@requires q.shape[-1] == k.shape[-1] if q.dim() >= 2
    return s


if __name__ == "__main__":
    q = torch.randn(2, 4, 5, 8)
    kv = torch.randn(2, 4, 7, 8)
    out = attention(q, kv, kv)  #@shape out: [2, 4, 5, 8]
    out2 = attention(q, kv, kv, mask=torch.ones(5, 7))  #@shape out2: [2, 4, 5, 8]
    s = attention_unknown_rank(torch.randn(3, 5, 8), torch.randn(7, 8))  #@shape s: [3, 5, 7]

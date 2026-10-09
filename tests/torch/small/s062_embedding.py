# nn.Embedding / F.embedding: indices of any rank gain a trailing embedding dim
import torch
import torch.nn.functional as F
from torch import nn


def embed_any(idx):
    emb = nn.Embedding(100, 16)
    e = emb(idx)  #@shape e: [*idx.shape, 16]
    #@requires none
    return e


def embed_params(idx, vocab, d):
    emb = nn.Embedding(vocab, d)
    e = emb(idx)  #@shape e: [*idx.shape, d]
    w = emb.weight  #@shape w: [vocab, d]
    return e, w


def tok_and_pos(ids):
    B, T = ids.shape
    tok = nn.Embedding(50, 8)
    pos = nn.Embedding(64, 8)
    p = pos(torch.arange(T))  #@shape p: [ids.shape[1], 8]
    x = tok(ids) + p  #@shape x: [ids.shape[0], ids.shape[1], 8]
    return x


def functional(idx):
    weight = torch.randn(10, 4)
    e = F.embedding(idx, weight)  #@shape e: [*idx.shape, 4]
    return e


def concrete():
    emb = nn.Embedding(100, 16)
    a = emb(torch.tensor(3))  #@shape a: [16]
    b = emb(torch.randint(0, 100, (2, 3, 4)))  #@shape b: [2, 3, 4, 16]
    return a, b


if __name__ == "__main__":
    embed_any(torch.randint(0, 100, (5, 2)))
    embed_params(torch.randint(0, 7, (3,)), 7, 5)
    tok_and_pos(torch.randint(0, 50, (2, 6)))
    functional(torch.randint(0, 10, (2, 2, 3)))
    concrete()

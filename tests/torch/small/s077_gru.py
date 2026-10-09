# nn.GRU: batch_first on unknown rank, bidirectional 2-layer h_n, last-layer states joined, unbatched, mismatch
import torch
from torch import nn


def gru_bf(x):
    gru = nn.GRU(16, 32, batch_first=True)
    out, h = gru(x)  #@shape out: [*x.shape[:-1], 32]
    #@requires x.dim() in (2, 3); x.shape[-1] == 16
    return out, h


def gru_known(x):
    B, T, D = x.shape
    gru = nn.GRU(D, 24, num_layers=2, bidirectional=True, batch_first=True)
    out, h = gru(x)  #@shape out: [x.shape[0], x.shape[1], 48]; h: [4, x.shape[0], 24]
    #@requires none
    final = torch.cat([h[-2], h[-1]], dim=1)  #@shape final: [x.shape[0], 48]
    return out, final


def gru_unbatched(T):
    x = torch.randn(T, 6)
    out, h = nn.GRU(6, 3, num_layers=2)(x)  #@shape out: [T, 3]; h: [2, 3]
    return out, h


def bad_features():
    gru = nn.GRU(16, 32)
    out, h = gru(torch.randn(5, 2, 8))  #@shape-error
    return out


if __name__ == "__main__":
    gru_bf(torch.randn(2, 5, 16))
    gru_known(torch.randn(3, 4, 7))
    gru_unbatched(5)
    try:
        bad_features()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

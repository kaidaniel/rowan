# nn.RNN: seq-first, batch_first on unknown rank, unbatched 2-D input, num_layers in h_n, 4-D input error
import torch
from torch import nn


def rnn_seq_first(x):
    T, N, I = x.shape
    rnn = nn.RNN(I, 16)
    out, h = rnn(x)  #@shape out: [x.shape[0], x.shape[1], 16]; h: [1, x.shape[1], 16]
    #@requires none
    return out, h


def rnn_batch_first(x):
    rnn = nn.RNN(8, 16, num_layers=3, batch_first=True)
    out, h = rnn(x)  #@shape out: [*x.shape[:-1], 16]
    #@requires x.dim() in (2, 3); x.shape[-1] == 8
    return out, h


def rnn_known_bf(x):
    N, T, I = x.shape
    out, h = nn.RNN(I, 16, num_layers=2, batch_first=True)(x)  #@shape out: [x.shape[0], x.shape[1], 16]; h: [2, x.shape[0], 16]
    return out, h


def rnn_unbatched(T):
    x = torch.randn(T, 8)
    out, h = nn.RNN(8, 4)(x)  #@shape out: [T, 4]; h: [1, 4]
    return out, h


def bad_rank():
    x = torch.randn(2, 3, 4, 8)
    out, h = nn.RNN(8, 16)(x)  #@shape-error
    return out


if __name__ == "__main__":
    rnn_seq_first(torch.randn(5, 2, 3))
    rnn_batch_first(torch.randn(2, 5, 8))
    rnn_batch_first(torch.randn(5, 8))
    rnn_known_bf(torch.randn(3, 4, 6))
    rnn_unbatched(7)
    try:
        bad_rank()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

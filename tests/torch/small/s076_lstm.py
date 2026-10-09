# nn.LSTM: num_layers and bidirectional in out / h_n / c_n, (h_n, c_n) tuple unpacking, sizes from parameters
import torch
from torch import nn


def lstm_bi(x):
    T, N, I = x.shape
    lstm = nn.LSTM(I, 32, num_layers=2, bidirectional=True)
    out, (h_n, c_n) = lstm(x)  #@shape out: [x.shape[0], x.shape[1], 64]; c_n: [4, x.shape[1], 32]
    return out, h_n, c_n


def lstm_params(x, i, h, L):
    N, T, _ = x.shape
    lstm = nn.LSTM(i, h, num_layers=L, batch_first=True)
    out, (hn, cn) = lstm(x)  #@shape out: [x.shape[0], x.shape[1], h]; hn: [L, x.shape[0], h]
    #@requires x.shape[2] == i
    last = out[:, -1, :]  #@shape last: [x.shape[0], h]
    return last, cn


def lstm_bi_params(x, h):
    lstm = nn.LSTM(10, h, bidirectional=True)
    out, state = lstm(x)  #@shape out: [*x.shape[:-1], 2*h]
    #@requires x.dim() in (2, 3); x.shape[-1] == 10
    return out, state


def concrete():
    out, (h, c) = nn.LSTM(10, 20, 2)(torch.randn(5, 3, 10))  #@shape c: [2, 3, 20]
    return out, h, c


if __name__ == "__main__":
    lstm_bi(torch.randn(6, 2, 5))
    lstm_params(torch.randn(3, 7, 4), 4, 5, 2)
    lstm_bi_params(torch.randn(4, 2, 10), 3)
    concrete()

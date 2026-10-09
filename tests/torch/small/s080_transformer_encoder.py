# nn.TransformerEncoderLayer / nn.TransformerEncoder keep the shape and need the last dim to be d_model
import torch
from torch import nn


def enc_layer(x):
    layer = nn.TransformerEncoderLayer(d_model=32, nhead=4, dim_feedforward=64, batch_first=True)
    y = layer(x)  #@shape y: [*x.shape[:-1], 32]
    #@requires x.dim() in (2, 3); x.shape[-1] == 32
    return y


def encoder(x):
    B, T, C = x.shape
    layer = nn.TransformerEncoderLayer(64, 8, batch_first=True)
    enc = nn.TransformerEncoder(layer, num_layers=2)
    y = enc(x)  #@shape y: [x.shape[0], x.shape[1], 64]
    #@requires x.shape[2] == 64
    pooled = y.mean(dim=1)  #@shape pooled: [x.shape[0], 64]
    return pooled


def concrete():
    layer = nn.TransformerEncoderLayer(16, 2, dim_feedforward=32)
    y = nn.TransformerEncoder(layer, num_layers=3)(torch.randn(5, 2, 16))  #@shape y: [5, 2, 16]
    return y


def bad_width():
    layer = nn.TransformerEncoderLayer(d_model=32, nhead=4, batch_first=True)
    y = layer(torch.randn(2, 5, 16))  #@shape-error
    return y


if __name__ == "__main__":
    enc_layer(torch.randn(2, 5, 32))
    encoder(torch.randn(2, 3, 64))
    concrete()
    try:
        bad_width()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

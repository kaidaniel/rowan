# nn.Sequential built inline: chaining, indexing seq[i], nested Sequential, sizes from parameters
import torch
from torch import nn


def mlp(x, d_in, d_out):
    net = nn.Sequential(nn.Linear(d_in, 64), nn.ReLU(), nn.Linear(64, d_out))
    y = net(x)  #@shape y: [*x.shape[:-1], d_out]
    #@requires x.shape[-1] == d_in
    first = net[0](x)  #@shape first: [*x.shape[:-1], 64]
    #@requires none
    n = net[2].out_features  #@dim n: d_out
    return y, first, n


def features_then_head(N):
    x = torch.randn(N, 3, 8, 8)
    feats = nn.Sequential(nn.Conv2d(3, 8, 3, padding=1), nn.ReLU(), nn.MaxPool2d(2))
    head = nn.Sequential(nn.Flatten(), nn.Linear(8 * 4 * 4, 10))
    h = feats(x)  #@shape h: [N, 8, 4, 4]
    y = head(h)  #@shape y: [N, 10]
    #@requires none
    return y


def nested(x):
    net = nn.Sequential(nn.Sequential(nn.Linear(4, 8), nn.GELU()), nn.Dropout(0.1), nn.Linear(8, 2))
    y = net(x)  #@shape y: [*x.shape[:-1], 2]
    #@requires x.shape[-1] == 4
    return y


def bad_inner():
    net = nn.Sequential(nn.Linear(4, 8), nn.ReLU(), nn.Linear(16, 2))
    y = net(torch.randn(3, 4))  #@shape-error
    return y


if __name__ == "__main__":
    mlp(torch.randn(2, 3, 5), 5, 7)
    features_then_head(2)
    nested(torch.randn(6, 4))
    try:
        bad_inner()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

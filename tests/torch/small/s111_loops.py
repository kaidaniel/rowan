# loops over range(n): a shape-preserving layer keeps the shape, torch.cat accumulation makes the dim ?
import torch
import torch.nn as nn


def deep(x, n):
    B, T, C = x.shape
    #@requires x.dim() == 3
    lin = nn.Linear(C, C)
    y = x
    for _ in range(n):
        y = torch.relu(lin(y))  #@shape y: [x.shape[0], x.shape[1], x.shape[2]]
        #@requires none
    out = y  #@shape out: [x.shape[0], x.shape[1], x.shape[2]]
    return out


def grow(x, n):
    B, T, C = x.shape
    y = x
    for _ in range(n):
        y = torch.cat([y, x], dim=1)  #@shape y: [x.shape[0], ?, x.shape[2]]
        #@requires none
    out = y  #@shape out: [x.shape[0], ?, x.shape[2]]
    return out


def residual_stack(x, n):
    B, T, C = x.shape
    layers = [nn.Linear(C, C) for _ in range(n)]
    h = x
    for layer in layers:
        h = h + layer(h)  #@shape h: [x.shape[0], x.shape[1], x.shape[2]]
        #@requires none
    out = h  #@shape out: [x.shape[0], x.shape[1], x.shape[2]]
    return out


def residual_unknown(x, layers):
    B, T, C = x.shape
    h = x
    for layer in layers:
        h = h + layer(h)
    # layer(h) is an unknown call: its result may broadcast h to a larger shape
    out = h  #@shape out: [*, ?, ?, ?]
    return out


if __name__ == "__main__":
    a = deep(torch.randn(2, 3, 4), 3)  #@shape a: [2, 3, 4]
    b = grow(torch.randn(2, 3, 4), 2)  #@shape b: [2, 9, 4]
    c = residual_stack(torch.randn(2, 3, 4), 2)  #@shape c: [2, 3, 4]
    d = residual_unknown(torch.randn(2, 3, 4), [nn.Linear(4, 4), nn.Tanh()])  #@shape d: [2, 3, 4]

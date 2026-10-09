# a parameter rebound (x = x.view(...)) keeps dims in the original tensor's terms (checks use the saved x0)
import torch
import torch.nn as nn


def rebind(x, n_heads):
    x0 = x
    B, T, C = x.shape
    x = x.view(B, T, n_heads, C // n_heads)  #@shape x: [x0.shape[0], x0.shape[1], n_heads, x0.shape[2] // n_heads]
    y = x.transpose(1, 2)  #@shape y: [x0.shape[0], n_heads, x0.shape[1], x0.shape[2] // n_heads]
    #@requires none
    x = x.reshape(B, T, C)  #@shape x: [x0.shape[0], x0.shape[1], x0.shape[2]]
    z = x + x0  #@shape z: [x0.shape[0], x0.shape[1], x0.shape[2]]
    #@requires none
    x = x.flatten(1)  #@shape x: [x0.shape[0], x0.shape[1]*x0.shape[2]]
    w = torch.randn(T * C, 5)
    o = x @ w  #@shape o: [x0.shape[0], 5]
    #@requires none
    return y, z, o


def rebind_req(x, d):
    x0 = x
    x = torch.relu(x) * 2
    lin = nn.Linear(d, 3)
    y = lin(x)  #@shape y: [*x0.shape[:-1], 3]
    #@requires x0.shape[-1] == d
    t = x0 + 0  #@shape t: [*x0.shape[:-1], d]
    return y, t


if __name__ == "__main__":
    y, z, o = rebind(torch.randn(2, 3, 8), 4)  #@shape y: [2, 4, 3, 2]; z: [2, 3, 8]; o: [2, 5]
    y2, t = rebind_req(torch.randn(5, 6), 6)  #@shape y2: [5, 3]; t: [5, 6]

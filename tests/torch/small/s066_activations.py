# Activation modules, Identity and Dropout keep the shape; a Linear after them still needs its last dim
import torch
from torch import nn


def chain(x):
    a = nn.ReLU()(x)  #@shape a: [*x.shape]
    #@requires none
    b = nn.GELU()(a)
    c = nn.Tanh()(b)
    d = nn.Sigmoid()(c)
    e = nn.SiLU()(d)  #@shape e: [*x.shape]
    f = nn.Identity()(e)
    g = nn.Dropout(0.1)(f)  #@shape g: [*x.shape]
    return g


def known(x):
    B, T = x.shape
    y = nn.Dropout(p=0.5)(nn.ReLU()(x))  #@shape y: [x.shape[0], x.shape[1]]
    return y


def act_then_linear(x):
    h = nn.SiLU()(x)
    y = nn.Linear(8, 2)(h)  #@shape y: [*x.shape[:-1], 2]
    #@requires x.shape[-1] == 8
    return y


def concrete():
    a = nn.GELU(approximate="tanh")(torch.randn(2, 3, 4))  #@shape a: [2, 3, 4]
    b = nn.Identity(54, unused=1)(torch.randn(7))  #@shape b: [7]
    return a, b


if __name__ == "__main__":
    chain(torch.randn(2, 3))
    known(torch.randn(4, 5))
    act_then_linear(torch.randn(3, 8))
    concrete()

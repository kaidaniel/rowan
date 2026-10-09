# unary elementwise ops keep the shape: clamp, exp, sqrt, abs, pow, **, unary minus, tanh/sigmoid/relu, log1p
import torch


def unary(x):
    a = torch.clamp(x, min=0)  #@shape a: [*x.shape]
    b = x.clamp(-1, 1)  #@shape b: [*x.shape]
    c = torch.exp(x)  #@shape c: [*x.shape]
    d = x.abs().sqrt()  #@shape d: [*x.shape]
    e = x.pow(2) + torch.pow(x, 3)  #@shape e: [*x.shape]
    f = -x  #@shape f: [*x.shape]
    g = x ** 0.5  #@shape g: [*x.shape]
    h = torch.tanh(torch.sigmoid(x))  #@shape h: [*x.shape]
    i = torch.relu(x).log1p()  #@shape i: [*x.shape]
    return a, b, c, d, e, f, g, h, i


def known(x):
    B, C = x.shape
    #@requires x.dim() == 2
    y = torch.exp(-x).clamp(max=1.0).rsqrt()  #@shape y: [x.shape[0], x.shape[1]]
    return y


if __name__ == "__main__":
    unary(torch.randn(2, 3))
    known(torch.randn(4, 5))

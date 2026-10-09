# elementwise ops on operands of the same shape (+ - * / // % **, torch.add/sub/mul) keep that shape
import torch


def same(x):
    a = x + x  #@shape a: [*x.shape]
    b = x * x - x  #@shape b: [*x.shape]
    c = torch.add(x, x, alpha=2)  #@shape c: [*x.shape]
    d = torch.mul(x, x) / (x * x + 1)  #@shape d: [*x.shape]
    return a, b, c, d


def known(x):
    B, T, C = x.shape
    #@requires x.dim() == 3
    y = torch.randn(B, T, C)
    z = x * y + y / 2  #@shape z: [x.shape[0], x.shape[1], x.shape[2]]
    #@requires none
    w = x // 2 % 3  #@shape w: [x.shape[0], x.shape[1], x.shape[2]]
    p = y ** 2 - torch.sub(x, y)  #@shape p: [x.shape[0], x.shape[1], x.shape[2]]
    return z, w, p


if __name__ == "__main__":
    same(torch.randn(2, 3))
    known(torch.randn(2, 3, 4))

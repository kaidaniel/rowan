# Python int arithmetic on dims: int(C / 2), C // 2 * 2, math.ceil, round (half to even), abs, negative //, %
import math
import torch
import torch.nn.functional as F


def halves(x):
    B, C = x.shape
    h = int(C / 2)  #@dim h: x.shape[1] // 2
    e = C // 2 * 2  #@dim e: 2 * (x.shape[1] // 2)
    y = x.view(B, 2, h)  #@shape y: [x.shape[0], 2, x.shape[1] // 2]
    q = x[:, :e]  #@shape q: [x.shape[0], min(2 * (x.shape[1] // 2), x.shape[1])]
    return y, q


def patches(x, p):
    B, T = x.shape
    n = math.ceil(T / p)
    pad = n * p - T
    xp = F.pad(x, (0, pad))  #@shape xp: [x.shape[0], _]
    y = xp.view(B, n, p)  #@shape y: [x.shape[0], _, p]
    return y


def concrete():
    n = math.ceil(10 / 4)  #@dim n: 3
    a = torch.zeros(n, 2)  #@shape a: [3, 2]
    k = round(7 / 2)  #@dim k: 4
    k2 = round(5 / 2)  #@dim k2: 2
    ab = abs(3 - 5)  #@dim ab: 2
    i = int(10 / 4)  #@dim i: 2
    f = 10 // 4 * 4  #@dim f: 8
    neg = -7 // 2  #@dim neg: -4
    md = -7 % 3  #@dim md: 2
    b = torch.zeros(k, k2, ab, i)  #@shape b: [4, 2, 2, 2]
    x = torch.randn(2, 10)
    c = x.view(2, int(x.shape[1] / 5), 5)  #@shape c: [2, 2, 5]
    t = x.shape[1] / 4
    m = torch.zeros(2, int(t))  #@shape m: [2, 2]
    r = torch.zeros(round(t))  #@shape r: [2]
    w = torch.zeros(f - md * 3)  #@shape w: [2]
    return a, b, c, m, r, w


def float_dim():
    x = torch.randn(2, 10)
    z = torch.zeros(2, x.shape[1] / 2)  #@shape-error
    return z


if __name__ == "__main__":
    y, q = halves(torch.randn(3, 6))  #@shape y: [3, 2, 3]; q: [3, 6]
    p = patches(torch.randn(2, 10), 4)  #@shape p: [2, 3, 4]
    concrete()
    try:
        float_dim()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

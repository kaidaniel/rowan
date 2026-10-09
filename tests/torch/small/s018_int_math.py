# Python int math on dims: products, floor division, min/max of ints, sizes built from them
import torch


def ints(x, k):
    N, C, L = x.shape
    #@requires x.dim() == 3
    half = C // 2  #@dim half: x.shape[1] // 2
    total = N * C * L  #@dim total: x.shape[0] * x.shape[1] * x.shape[2]
    flat = torch.zeros(N, C * L)  #@shape flat: [x.shape[0], x.shape[1] * x.shape[2]]
    m = max(L, k)  #@dim m: max(x.shape[2], k)
    y = torch.zeros(N, m)  #@shape y: [x.shape[0], max(x.shape[2], k)]
    s = min(C, L)  #@dim s: min(x.shape[1], x.shape[2])
    z = torch.ones(s, k + L)  #@shape z: [min(x.shape[1], x.shape[2]), k + x.shape[2]]
    return half, total, flat, y, z


def concrete():
    a = 7
    b = (a - 1) // 2 + a * 3  #@dim b: 24
    t = torch.zeros(max(a, 3), min(a, 3), b - 20)  #@shape t: [7, 3, 4]
    return t


if __name__ == "__main__":
    ints(torch.randn(2, 6, 5), 3)
    ints(torch.randn(2, 4, 3), 9)
    concrete()

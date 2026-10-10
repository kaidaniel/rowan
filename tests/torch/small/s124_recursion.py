# recursion over a tensor: halving until length 1 must not claim the input length; shape-preserving recursion keeps it
import torch
import torch.nn.functional as F


def halve(x):
    if x.shape[-1] <= 1:
        return x
    return halve(x[..., ::2])


def run(x):
    B, T = x.shape
    out = halve(x)  #@shape out: [x.shape[0], ?]
    return out


def apply_n(x, n):
    if n == 0:
        return x
    return apply_n(torch.relu(x) + x, n - 1)


def run2(x, n):
    y = apply_n(x, n)  #@shape y: [*x.shape]
    return y


def pyramid(x, levels):
    if levels == 0:
        return [x]
    return [x] + pyramid(F.avg_pool1d(x, 2), levels - 1)


def run3(x):
    B, C, L = x.shape
    feats = pyramid(x, 2)
    f0 = feats[0]  #@shape f0: [x.shape[0], x.shape[1], x.shape[2]]
    n = len(feats)  #@dim n: 3
    last = feats[-1]  #@shape last: [x.shape[0], x.shape[1], x.shape[2] // 4]
    return f0, last


if __name__ == "__main__":
    # (precise by following the recursion 5 calls deep: a tensor of a known shape is a distinct
    # activation per shape, like a literal argument)
    a = run(torch.randn(2, 9))  #@shape a: [2, 1]
    b = run2(torch.randn(3, 4), 3)  #@shape b: [3, 4]
    f0, last = run3(torch.randn(2, 3, 16))  #@shape f0: [2, 3, 16]; last: [2, 3, 4]

# generators yielding tensors with next(), zip over lists of tensors keeps each position's shape
import torch


def chunks(x, n):
    for i in range(n):
        yield x[:, i]


def first_chunk(x):
    B, T, C = x.shape
    g = chunks(x, 3)
    a = next(g)  #@shape a: [x.shape[0], x.shape[2]]
    b = next(g)  #@shape b: [x.shape[0], x.shape[2]]
    gen = (x[:, i] * 2 for i in range(T))
    c = next(gen)  #@shape c: [x.shape[0], x.shape[2]]
    d = next(iter([x.sum(dim=1), x]))  #@shape d: [x.shape[0], x.shape[2]]
    return a, b, c, d


def zipped(x):
    B, T, C = x.shape
    qs = [x, x.transpose(1, 2)]
    ks = [x.sum(dim=1), x.sum(dim=2)]
    (q0, k0), (q1, k1) = zip(qs, ks)
    #@shape q1: [x.shape[0], x.shape[2], x.shape[1]]; k1: [x.shape[0], x.shape[1]]; k0: [x.shape[0], x.shape[2]]
    sums = []
    for q, k in zip(qs, ks):
        s = q.sum(dim=-1)  #@shape s: [x.shape[0], ?]
        sums.append(s)
    pairs = list(zip([x, x], [k0, k0]))
    p = pairs[1][1]  #@shape p: [x.shape[0], x.shape[2]]
    e = [a * 2 for a, _ in zip(qs, ks)][1]  #@shape e: [x.shape[0], x.shape[2], x.shape[1]]
    return sums, p, e


if __name__ == "__main__":
    a, b, c, d = first_chunk(torch.randn(2, 5, 3))  #@shape a: [2, 3]; d: [2, 3]
    sums, p, e = zipped(torch.randn(2, 3, 4))  #@shape p: [2, 4]; e: [2, 4, 3]

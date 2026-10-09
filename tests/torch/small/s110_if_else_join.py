# if/else on an unknown flag: equal shapes stay, different dims join to ?, different ranks keep the suffix
import torch


def pick(x, flag):
    B, T, C = x.shape
    if flag:
        y = x.mean(dim=1)  #@shape y: [x.shape[0], x.shape[2]]
    else:
        y = x[:, -1]  #@shape y: [x.shape[0], x.shape[2]]
    out = y  #@shape out: [x.shape[0], x.shape[2]]
    return out


def pool(x, mode):
    B, T, C = x.shape
    if mode == "flat":
        y = x.reshape(B, T * C)  #@shape y: [x.shape[0], x.shape[1]*x.shape[2]]
    else:
        y = x.mean(1)
    out = y  #@shape out: [x.shape[0], ?]
    return out


def maybe_batch(x, flag):
    N, D = x.shape
    if flag:
        y = x.unsqueeze(0)  #@shape y: [1, x.shape[0], x.shape[1]]
    else:
        y = x
    out = y  #@shape out: [*, x.shape[0], x.shape[1]]
    return out


if __name__ == "__main__":
    x = torch.randn(2, 3, 4)
    a = pick(x, True)  #@shape a: [2, 4]
    b = pick(x, False)  #@shape b: [2, 4]
    c = pool(x, "flat")  #@shape c: [2, 12]
    d = pool(x, "mean")  #@shape d: [2, 4]
    e = maybe_batch(torch.randn(5, 6), True)  #@shape e: [1, 5, 6]
    f = maybe_batch(torch.randn(5, 6), False)  #@shape f: [5, 6]

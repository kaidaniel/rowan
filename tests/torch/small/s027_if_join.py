# joins over if/else: equal shapes stay, differing dims become ?, differing ranks keep the common trailing dims
import torch


def same(x, flag):
    if flag:
        y = x * 2
    else:
        y = x + 1
    z = y  #@shape z: [*x.shape]
    return z


def dims(n, flag):
    if flag:
        y = torch.zeros(n, 3)
    else:
        y = torch.zeros(n, 4)
    z = y  #@shape z: [n, ?]
    return z


def ranks(n, flag):
    if flag:
        y = torch.zeros(n, 5)
    else:
        y = torch.zeros(5)
    z = y  #@shape z: [*, 5]
    return z


def one_branch(x, flag):
    B, C = x.shape
    #@requires x.dim() == 2
    y = torch.zeros(B, C)
    if flag:
        y = torch.zeros(B, 2 * C)
    z = y  #@shape z: [x.shape[0], ?]
    w = torch.zeros(B, C) if flag else torch.ones(B, C)  #@shape w: [x.shape[0], x.shape[1]]
    return z, w


def decided():
    flag = True
    if flag:
        y = torch.zeros(2, 3)
    else:
        y = torch.zeros(4)
    z = y  #@shape z: [2, 3]
    return z


if __name__ == "__main__":
    same(torch.randn(2, 3), True)
    same(torch.randn(4), False)
    dims(2, True)
    dims(2, False)
    ranks(3, True)
    ranks(3, False)
    one_branch(torch.randn(2, 3), True)
    one_branch(torch.randn(2, 3), False)
    decided()

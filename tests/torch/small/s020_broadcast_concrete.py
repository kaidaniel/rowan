# broadcasting with literal shapes: [3,1]+[1,4], trailing-dim alignment, 0-d operands, size 0; mismatches are errors
import torch


def bc():
    a = torch.zeros(3, 1) + torch.zeros(1, 4)  #@shape a: [3, 4]
    b = torch.ones(2, 3, 4) * torch.ones(4)  #@shape b: [2, 3, 4]
    c = torch.ones(5, 1, 4) - torch.ones(3, 1)  #@shape c: [5, 3, 4]
    d = torch.tensor(2.0) + torch.ones(2, 3)  #@shape d: [2, 3]
    e = torch.zeros(1) * torch.zeros(6, 2)  #@shape e: [6, 2]
    f = torch.zeros(0, 3) + torch.zeros(1, 3)  #@shape f: [0, 3]
    return a, b, c, d, e, f


def bad():
    y = torch.ones(3, 4) + torch.ones(5)  #@shape-error
    return y


def bad2():
    y = torch.ones(2, 3) * torch.ones(3, 2)  #@shape-error
    return y


if __name__ == "__main__":
    bc()
    try:
        bad()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass
    try:
        bad2()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

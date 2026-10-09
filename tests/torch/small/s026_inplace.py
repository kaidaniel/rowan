# in-place ops keep the left operand's shape: +=, *=, add_, mul_, clamp_, zero_; the right side must broadcast to it
import torch


def inplace(n):
    a = torch.zeros(n, 3)
    a += 1  #@shape a: [n, 3]
    a.add_(torch.ones(3))  #@shape a: [n, 3]
    a *= torch.ones(n, 1)  #@shape a: [n, 3]
    b = a.mul_(2)  #@shape b: [n, 3]
    a.clamp_(min=0).zero_()  #@shape a: [n, 3]
    return a, b


def on_param(x):
    x += 1  #@shape x: [*x.shape]
    y = x.sub_(0.5)  #@shape y: [*x.shape]
    return y


def bad():
    a = torch.zeros(3)
    a += torch.ones(2, 3)  #@shape-error
    return a


def bad2():
    a = torch.zeros(4, 1)
    a.add_(torch.ones(4, 5))  #@shape-error
    return a


if __name__ == "__main__":
    inplace(4)
    on_param(torch.randn(2, 3))
    try:
        bad()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass
    try:
        bad2()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

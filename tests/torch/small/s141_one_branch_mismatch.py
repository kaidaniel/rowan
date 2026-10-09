# a definite mismatch inside one branch is an error there and that path does not reach the join; a mismatch on only one joined value is not definite
import torch


def only_one_branch(flag):
    a = torch.randn(4, 6)
    if flag:
        b = a @ torch.randn(6, 3)  #@shape b: [4, 3]
    else:
        b = a @ torch.randn(5, 3)  #@shape-error
    c = b + 1  #@shape c: [4, 3]
    return c


def joined(flag):
    a = torch.randn(4, 6)
    w = torch.randn(6, 3) if flag else torch.randn(5, 3)  #@shape w: [?, 3]
    c = a @ w  #@shape c: [4, 3]
    return c


def guarded(x):
    B, C = x.shape
    if C == 6:
        y = x @ torch.randn(6, 2)  #@shape y: [x.shape[0], 2]
        #@requires none
    else:
        y = x.new_zeros(B, 2)  #@shape y: [x.shape[0], 2]
    out = y  #@shape out: [x.shape[0], 2]
    return out


if __name__ == "__main__":
    c = only_one_branch(True)  #@shape c: [4, 3]
    try:
        only_one_branch(False)
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass
    j = joined(True)  #@shape j: [4, 3]
    g = guarded(torch.randn(3, 6))  #@shape g: [3, 2]
    g2 = guarded(torch.randn(3, 5))  #@shape g2: [3, 2]

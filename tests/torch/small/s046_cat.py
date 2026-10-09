# torch.cat: the cat dim sums symbolically, the other dims must be equal (requirements, then facts)
import torch


def cat_seq(a, b):
    B, T, C = a.shape  #@requires a.dim() == 3
    B2, S, C2 = b.shape  #@requires b.dim() == 3
    y = torch.cat([a, b], dim=1)  #@shape y: [a.shape[0], a.shape[1] + b.shape[1], a.shape[2]]
    #@requires a.shape[0] == b.shape[0]; a.shape[2] == b.shape[2]
    z = torch.cat((y, b), 1)  #@shape z: [a.shape[0], a.shape[1] + 2*b.shape[1], a.shape[2]]
    #@requires none
    return y, z


def cat_unknown(x):
    y = torch.cat([x, x])  #@shape y: [2*x.shape[0], *x.shape[1:]]
    #@requires none
    z = torch.cat([x, x, x], dim=-1)  #@shape z: [*x.shape[:-1], 3*x.shape[-1]]
    #@requires none
    return y, z


def prepend_token(x):
    B, T, C = x.shape  #@requires x.dim() == 3
    cls = torch.zeros(B, 1, C)
    y = torch.cat([cls, x], dim=1)  #@shape y: [x.shape[0], x.shape[1] + 1, x.shape[2]]
    #@requires none
    return y


def concrete():
    a = torch.cat([torch.zeros(2, 3), torch.zeros(4, 3)])  #@shape a: [6, 3]
    b = torch.cat([torch.zeros(2, 3), torch.zeros(2, 1)], dim=-1)  #@shape b: [2, 4]
    return a, b


def bad_cat():
    return torch.cat([torch.zeros(2, 3), torch.zeros(2, 4)])  #@shape-error


if __name__ == "__main__":
    cat_seq(torch.randn(2, 3, 4), torch.randn(2, 5, 4))
    cat_unknown(torch.randn(2, 3))
    cat_unknown(torch.randn(4))
    prepend_token(torch.randn(2, 3, 4))
    concrete()
    try:
        bad_cat()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

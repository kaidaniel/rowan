# torch.stack: a new dim of size len(list) at dim; all pieces must have equal shapes
import torch


def stack_same(x):
    a = torch.stack([x, x])  #@shape a: [2, *x.shape]
    b = torch.stack([x, x, x], dim=1)  #@shape b: [x.shape[0], 3, *x.shape[1:]]
    c = torch.stack((x, x), dim=-1)  #@shape c: [*x.shape, 2]
    #@requires none
    return a, b, c


def stack_pair(x, y):
    B, C = x.shape  #@requires x.dim() == 2
    B2, C2 = y.shape  #@requires y.dim() == 2
    s = torch.stack([x, y], dim=0)  #@shape s: [2, x.shape[0], x.shape[1]]
    #@requires x.shape[0] == y.shape[0]; x.shape[1] == y.shape[1]
    return s


def stack_loop(x):
    B, C = x.shape  #@requires x.dim() == 2
    outs = []
    for i in range(4):
        outs.append(x * i)
    s = torch.stack(outs, dim=1)  #@shape s: [x.shape[0], 4, x.shape[1]]
    return s


def bad_stack():
    return torch.stack([torch.zeros(2), torch.zeros(3)])  #@shape-error


if __name__ == "__main__":
    stack_same(torch.randn(2, 3))
    stack_same(torch.randn(5))
    stack_pair(torch.randn(2, 3), torch.randn(2, 3))
    stack_loop(torch.randn(2, 3))
    try:
        bad_stack()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

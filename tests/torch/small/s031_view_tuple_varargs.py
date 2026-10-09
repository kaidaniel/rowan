# view / reshape sizes as varargs, a tuple, a list, a torch.Size, x.size(i) and starred shape slices
import torch


def forms(x):
    B, T, C = x.shape  #@requires x.dim() == 3
    a = x.view(B, T * C)  #@shape a: [x.shape[0], x.shape[1]*x.shape[2]]
    b = x.view((B, T * C))  #@shape b: [x.shape[0], x.shape[1]*x.shape[2]]
    c = x.reshape([T, B, C])  #@shape c: [x.shape[1], x.shape[0], x.shape[2]]
    #@requires none
    d = a.view(x.shape)  #@shape d: [x.shape[0], x.shape[1], x.shape[2]]
    e = x.reshape(x.size(0), -1)  #@shape e: [x.shape[0], x.shape[1]*x.shape[2]]
    f = a.view(*x.shape[:2], C)  #@shape f: [x.shape[0], x.shape[1], x.shape[2]]
    g = x.view(B, T, 2, C // 2)  #@shape g: [x.shape[0], x.shape[1], 2, x.shape[2] // 2]
    #@requires none
    return a, b, c, d, e, f, g


def sizes_unknown(x):
    s = x.size()  #@shape s: [*x.shape]
    y = x.reshape(s)  #@shape y: [*x.shape]
    z = x.view(x.size(0), -1)  #@shape z: [x.shape[0], ?]
    return y, z


def concrete():
    t = torch.zeros(6)
    a = t.view(2, 3)  #@shape a: [2, 3]
    b = t.view((3, 2))  #@shape b: [3, 2]
    c = t.reshape([1, 6, 1])  #@shape c: [1, 6, 1]
    return a, b, c


def bad_view():
    t = torch.zeros(6)
    return t.view(4, 2)  #@shape-error


if __name__ == "__main__":
    forms(torch.randn(2, 3, 4))
    forms(torch.randn(1, 5, 6))
    sizes_unknown(torch.randn(3, 4, 5))
    sizes_unknown(torch.randn(6, 2))
    concrete()
    try:
        bad_view()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

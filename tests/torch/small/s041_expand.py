# expand: -1 keeps a dim, a literal-1 dim grows to any size, extra sizes add leading dims
import torch


def expand_mid(x, T):
    B, C = x.shape  #@requires x.dim() == 2
    a = x.unsqueeze(1).expand(-1, T, -1)  #@shape a: [x.shape[0], T, x.shape[1]]
    #@requires none
    b = x.unsqueeze(1).expand(B, T, C)  #@shape b: [x.shape[0], T, x.shape[1]]
    #@requires none
    c = x.expand(3, B, C)  #@shape c: [3, x.shape[0], x.shape[1]]
    d = x.expand(2, -1, -1)  #@shape d: [2, x.shape[0], x.shape[1]]
    e = x[:, None, :].expand((B, 4, C))  #@shape e: [x.shape[0], 4, x.shape[1]]
    return a, b, c, d, e


def expand_unknown(x):
    y = x.expand(4, *x.shape)  #@shape y: [4, *x.shape]
    #@requires none
    return y


def concrete():
    a = torch.zeros(3, 1).expand(3, 5)  #@shape a: [3, 5]
    b = torch.zeros(1, 4).expand(2, 6, -1)  #@shape b: [2, 6, 4]
    return a, b


def bad_expand():
    return torch.randn(2, 3).expand(2, 4)  #@shape-error


if __name__ == "__main__":
    expand_mid(torch.randn(2, 3), 5)
    expand_unknown(torch.randn(2, 3))
    expand_unknown(torch.randn(5))
    concrete()
    try:
        bad_expand()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

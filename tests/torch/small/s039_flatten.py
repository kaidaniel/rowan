# flatten(start, end) / torch.flatten(x, 1): products of the merged dims; unknown when they include a rest
import torch


def flat_known(x):
    N, C, H, W = x.shape  #@requires x.dim() == 4
    a = x.flatten(1)  #@shape a: [x.shape[0], x.shape[1]*x.shape[2]*x.shape[3]]
    b = torch.flatten(x, 1)  #@shape b: [x.shape[0], x.shape[1]*x.shape[2]*x.shape[3]]
    c = x.flatten(start_dim=2)  #@shape c: [x.shape[0], x.shape[1], x.shape[2]*x.shape[3]]
    d = x.flatten(1, 2)  #@shape d: [x.shape[0], x.shape[1]*x.shape[2], x.shape[3]]
    #@requires none
    e = x.flatten()  #@shape e: [x.shape[0]*x.shape[1]*x.shape[2]*x.shape[3]]
    f = x.flatten(-2, -1)  #@shape f: [x.shape[0], x.shape[1], x.shape[2]*x.shape[3]]
    g = x.flatten(end_dim=1)  #@shape g: [x.shape[0]*x.shape[1], x.shape[2], x.shape[3]]
    return a, b, c, d, e, f, g


def flat_unknown(x):
    a = x.flatten(-2)  #@shape a: [*x.shape[:-2], x.shape[-2]*x.shape[-1]]
    c = x.flatten(0, 1)  #@shape c: [x.shape[0]*x.shape[1], *x.shape[2:]]
    b = torch.flatten(x, 1)  #@shape b: [x.shape[0], ?]
    return a, b, c


def bad_start():
    return torch.randn(2, 3).flatten(2)  #@shape-error


if __name__ == "__main__":
    flat_known(torch.randn(2, 3, 4, 5))
    flat_unknown(torch.randn(2, 3, 4))
    flat_unknown(torch.randn(2, 3, 4, 5, 6))
    try:
        bad_start()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

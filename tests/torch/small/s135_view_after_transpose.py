# reshape / contiguous().view after transpose, -1 solved symbolically, a dim computed from numel(); a literal count that does not divide is an error
import torch


def merge_heads(x):
    B, H, T, D = x.shape
    y = x.transpose(1, 2).reshape(B, T, H * D)  #@shape y: [x.shape[0], x.shape[2], x.shape[1]*x.shape[3]]
    z = x.transpose(1, 2).contiguous().view(B, T, -1)  #@shape z: [x.shape[0], x.shape[2], x.shape[1]*x.shape[3]]
    f = x.reshape(B, -1)  #@shape f: [x.shape[0], x.shape[1]*x.shape[2]*x.shape[3]]
    n = x.numel()  #@dim n: x.shape[0]*x.shape[1]*x.shape[2]*x.shape[3]
    w = x.reshape(n // D, D)  #@shape w: [x.shape[0]*x.shape[1]*x.shape[2], x.shape[3]]
    return y, z, f, w


def split_heads(x, h):
    B, T, C = x.shape
    q = x.view(B, T, h, C // h).transpose(1, 2)  #@shape q: [x.shape[0], h, x.shape[1], x.shape[2] // h]
    back = q.transpose(1, 2).reshape(B, T, C)  #@shape back: [x.shape[0], x.shape[1], x.shape[2]]
    return back


def literal_bad():
    a = torch.randn(2, 3, 4).transpose(0, 2).reshape(4, 5)  #@shape-error
    return a


if __name__ == "__main__":
    y, z, f, w = merge_heads(torch.randn(2, 3, 5, 4))  #@shape y: [2, 5, 12]; z: [2, 5, 12]; f: [2, 60]; w: [30, 4]
    b = split_heads(torch.randn(2, 5, 12), 3)  #@shape b: [2, 5, 12]
    try:
        literal_bad()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

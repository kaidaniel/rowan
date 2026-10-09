# F.conv2d with a weight tensor (built or a parameter), F.max_pool2d / F.avg_pool2d / F.adaptive_avg_pool2d
import torch
import torch.nn.functional as F


def fconv(x):
    N, C, H, W = x.shape
    weight = torch.randn(16, C, 3, 3)
    y = F.conv2d(x, weight, padding=1)  #@shape y: [x.shape[0], 16, x.shape[2], x.shape[3]]
    #@requires none
    p = F.max_pool2d(y, 2)  #@shape p: [x.shape[0], 16, (x.shape[2] - 2)//2 + 1, (x.shape[3] - 2)//2 + 1]
    a = F.avg_pool2d(y, 3, stride=1, padding=1)  #@shape a: [x.shape[0], 16, x.shape[2], x.shape[3]]
    g = F.adaptive_avg_pool2d(y, 1)  #@shape g: [x.shape[0], 16, 1, 1]
    g2 = F.adaptive_avg_pool2d(y, (2, 3))  #@shape g2: [x.shape[0], 16, 2, 3]
    return p, a, g, g2


def fconv_weight_param(x, w):
    N, C, H, W = x.shape
    Co, Ci, kh, kw = w.shape
    y = F.conv2d(x, w)  #@shape y: [x.shape[0], w.shape[0], x.shape[2] - w.shape[2] + 1, x.shape[3] - w.shape[3] + 1]
    #@requires x.shape[1] == w.shape[1]
    s = F.conv2d(x, w, stride=2)  #@shape s: [x.shape[0], w.shape[0], (x.shape[2] - w.shape[2])//2 + 1, (x.shape[3] - w.shape[3])//2 + 1]
    #@requires none
    return y, s


def bad_weight():
    y = F.conv2d(torch.randn(1, 3, 8, 8), torch.randn(4, 2, 3, 3))  #@shape-error
    return y


if __name__ == "__main__":
    fconv(torch.randn(2, 3, 9, 8))
    fconv_weight_param(torch.randn(2, 3, 9, 8), torch.randn(5, 3, 3, 2))
    try:
        bad_weight()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

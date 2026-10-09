# a helper returning a tuple of tensors, unpacked by callers that are themselves analysed on their own
import torch


def stats(x):
    B, T, C = x.shape
    #@requires x.dim() == 3
    mu = x.mean(dim=-1, keepdim=True)  #@shape mu: [x.shape[0], x.shape[1], 1]
    mx, idx = x.max(dim=1)  #@shape mx: [x.shape[0], x.shape[2]]; idx: [x.shape[0], x.shape[2]]
    return mu, mx, idx


def ends(x):
    return x[:, 0], x[:, -1]


def normalize(x):
    mu, mx, idx = stats(x)  #@shape mu: [x.shape[0], x.shape[1], 1]; idx: [x.shape[0], x.shape[2]]
    y = x - mu  #@shape y: [x.shape[0], x.shape[1], x.shape[2]]
    #@requires none
    first, last = ends(y)  #@shape first: [x.shape[0], x.shape[2]]; last: [x.shape[0], x.shape[2]]
    both = first + last + mx  #@shape both: [x.shape[0], x.shape[2]]
    #@requires none
    return y, both


if __name__ == "__main__":
    y, both = normalize(torch.randn(2, 5, 3))  #@shape y: [2, 5, 3]; both: [2, 3]
    a, b = ends(torch.randn(4, 6))  #@shape a: [4]; b: [4]

# torch.where(cond, a, b) broadcasts all three; masked_fill keeps the broadcast shape; a causal mask on [B, T, T]
import torch


def where_(x):
    y = torch.where(x > 0, x, 0.0)  #@shape y: [*x.shape]
    z = torch.where(x > 0, x, torch.zeros_like(x))  #@shape z: [*x.shape]
    return y, z


def causal(x):
    B, T, C = x.shape
    #@requires x.dim() == 3
    att = torch.randn(B, T, T)
    mask = torch.tril(torch.ones(T, T))  #@shape mask: [x.shape[1], x.shape[1]]
    att = att.masked_fill(mask == 0, float("-inf"))  #@shape att: [x.shape[0], x.shape[1], x.shape[1]]
    #@requires none
    w = torch.where(mask.bool(), att, torch.zeros(1))  #@shape w: [x.shape[0], x.shape[1], x.shape[1]]
    return att, w


def concrete():
    c = torch.where(torch.zeros(3, 1) > 0, torch.ones(1, 4), torch.tensor(0.0))  #@shape c: [3, 4]
    m = torch.zeros(2, 3).masked_fill(torch.ones(3, dtype=torch.bool), 1.0)  #@shape m: [2, 3]
    return c, m


def bad():
    m = torch.zeros(2, 3).masked_fill(torch.ones(4, dtype=torch.bool), 1.0)  #@shape-error
    return m


if __name__ == "__main__":
    where_(torch.randn(2, 3))
    causal(torch.randn(2, 4, 8))
    concrete()
    try:
        bad()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

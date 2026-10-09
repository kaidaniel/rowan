# helpers building masks: causal_mask(T), padding_mask(lengths, T), apply_causal(att, T)
import torch


def causal_mask(T):
    ones = torch.ones(T, T)  #@shape ones: [T, T]
    m = torch.tril(ones)  #@shape m: [T, T]
    return m.bool()


def padding_mask(lengths, T):
    assert lengths.dim() == 1
    ar = torch.arange(T)  #@shape ar: [T]
    mask = ar[None, :] < lengths[:, None]  #@shape mask: [lengths.shape[0], T]
    #@requires none
    return mask


def apply_causal(att, T):
    m = causal_mask(T)  #@shape m: [T, T]
    out = att.masked_fill(~m, 0.0)  #@shape out: [*broadcast(att.shape, [T, T])]
    return out


def apply_causal_3d(att):
    B, T, S = att.shape
    out = att.masked_fill(~causal_mask(T), 0.0)  #@shape out: [att.shape[0], att.shape[1], max(att.shape[1], att.shape[2])]
    return out


if __name__ == "__main__":
    m = causal_mask(5)  #@shape m: [5, 5]
    pm = padding_mask(torch.tensor([3, 5, 2]), 6)  #@shape pm: [3, 6]
    a = apply_causal(torch.randn(2, 4, 4), 4)  #@shape a: [2, 4, 4]
    b = apply_causal_3d(torch.randn(2, 4, 4))  #@shape b: [2, 4, 4]

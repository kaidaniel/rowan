# nanoGPT head split / merge: view(B, T, n_head, C // n_head).transpose(1, 2) with n_head a parameter
import torch


def split_heads(x, n_head):
    B, T, C = x.size()  #@requires x.dim() == 3
    h = x.view(B, T, n_head, C // n_head)  #@shape h: [x.shape[0], x.shape[1], n_head, x.shape[2] // n_head]
    #@requires none
    q = h.transpose(1, 2)  #@shape q: [x.shape[0], n_head, x.shape[1], x.shape[2] // n_head]
    #@requires none
    return q


def merge_heads(y, n_embd):
    B, nh, T, hs = y.shape  #@requires y.dim() == 4
    m = y.transpose(1, 2).contiguous().view(B, T, n_embd)  #@shape m: [y.shape[0], y.shape[2], n_embd]
    m2 = y.transpose(1, 2).reshape(B, T, nh * hs)  #@shape m2: [y.shape[0], y.shape[2], y.shape[1]*y.shape[3]]
    return m, m2


def attention_scores(x, n_head):
    B, T, C = x.shape
    q = x.view(B, T, n_head, C // n_head).transpose(1, 2)
    k = x.view(B, T, n_head, C // n_head).transpose(1, 2)
    att = q @ k.transpose(-2, -1)  #@shape att: [x.shape[0], n_head, x.shape[1], x.shape[1]]
    #@requires none
    y = att @ q  #@shape y: [x.shape[0], n_head, x.shape[1], x.shape[2] // n_head]
    #@requires none
    out = y.transpose(1, 2).contiguous().view(B, T, C)  #@shape out: [x.shape[0], x.shape[1], x.shape[2]]
    return out


if __name__ == "__main__":
    split_heads(torch.randn(2, 5, 12), 3)
    split_heads(torch.randn(1, 1, 8), 8)
    merge_heads(torch.randn(2, 3, 5, 4), 12)
    attention_scores(torch.randn(2, 5, 12), 4)

# patchify(x, p): view into patches, permute, reshape to tokens; a literal view mismatch
import torch


def patchify(x, p):
    B, C, H, W = x.shape
    #@requires x.dim() == 4
    t = x.view(B, C, H // p, p, W // p, p)  #@shape t: [x.shape[0], x.shape[1], x.shape[2]//p, p, x.shape[3]//p, p]
    #@requires none
    t = t.permute(0, 2, 4, 3, 5, 1)  #@shape t: [x.shape[0], x.shape[2]//p, x.shape[3]//p, p, p, x.shape[1]]
    out = t.reshape(B, (H // p) * (W // p), p * p * C)  #@shape out: [x.shape[0], (x.shape[2]//p)*(x.shape[3]//p), p*p*x.shape[1]]
    n = out.shape[1]  #@dim n: (x.shape[2]//p)*(x.shape[3]//p)
    return out


def unpatchify(tokens, p, h, w):
    B, N, D = tokens.shape
    #@requires tokens.dim() == 3
    t = tokens.reshape(B, h, w, p, p, -1)  #@shape t: [tokens.shape[0], h, w, p, p, ?]
    img = t.permute(0, 5, 1, 3, 2, 4).reshape(B, -1, h * p, w * p)  #@shape img: [tokens.shape[0], ?, h*p, w*p]
    return img


def bad_view():
    x = torch.randn(2, 3, 8, 8)
    y = x.view(2, 3, 3, 3)  #@shape-error
    return y


if __name__ == "__main__":
    tok = patchify(torch.randn(2, 3, 8, 8), 4)  #@shape tok: [2, 4, 48]
    img = unpatchify(tok, 4, 2, 2)  #@shape img: [2, 3, 8, 8]
    try:
        bad_view()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

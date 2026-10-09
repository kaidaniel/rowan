# F.scaled_dot_product_attention: (..., L, E) x (..., S, E) x (..., S, Ev) -> (..., L, Ev); heads split from x
import torch
import torch.nn.functional as F


def sdpa_known(B, H, T, D):
    q = torch.randn(B, H, T, D)
    k = torch.randn(B, H, T, D)
    v = torch.randn(B, H, T, D)
    y = F.scaled_dot_product_attention(q, k, v, is_causal=True)  #@shape y: [B, H, T, D]
    #@requires none
    return y


def sdpa_cross(B, L, S, E, Ev):
    q = torch.randn(B, L, E)
    k = torch.randn(B, S, E)
    v = torch.randn(B, S, Ev)
    y = F.scaled_dot_product_attention(q, k, v)  #@shape y: [B, L, Ev]
    #@requires none
    return y


def sdpa_heads(x, n_head):
    B, T, C = x.shape
    q = x.view(B, T, n_head, C // n_head).transpose(1, 2)  #@shape q: [x.shape[0], n_head, x.shape[1], x.shape[2]//n_head]
    y = F.scaled_dot_product_attention(q, q, q, is_causal=True)  #@shape y: [x.shape[0], n_head, x.shape[1], x.shape[2]//n_head]
    out = y.transpose(1, 2).reshape(B, T, C)  #@shape out: [x.shape[0], x.shape[1], x.shape[2]]
    return out


def sdpa_unknown(q, k, v):
    y = F.scaled_dot_product_attention(q, k, v)  #@shape y: [*_, q.shape[-2], v.shape[-1]]
    return y


def bad_head_dim():
    q = torch.randn(2, 4, 8)
    k = torch.randn(2, 4, 16)
    y = F.scaled_dot_product_attention(q, k, k)  #@shape-error
    return y


if __name__ == "__main__":
    sdpa_known(2, 3, 4, 5)
    sdpa_cross(2, 3, 6, 4, 7)
    sdpa_heads(torch.randn(2, 5, 12), 3)
    sdpa_unknown(torch.randn(2, 3, 4), torch.randn(2, 5, 4), torch.randn(2, 5, 6))
    try:
        bad_head_dim()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

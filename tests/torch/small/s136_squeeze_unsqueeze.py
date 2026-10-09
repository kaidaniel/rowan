# squeeze on literal 1 dims is decided; squeeze of a symbolic dim may or may not drop it (rank unknown); unsqueeze/None indexing insert at the right place
import torch


def literal():
    a = torch.randn(3, 1, 4, 1)
    b = a.squeeze()  #@shape b: [3, 4]
    c = a.squeeze(1)  #@shape c: [3, 4, 1]
    d = a.squeeze(0)  #@shape d: [3, 1, 4, 1]
    e = a.squeeze(-1).unsqueeze(0)  #@shape e: [1, 3, 1, 4]
    f = a.squeeze((1, 3))  #@shape f: [3, 4]
    g = a[..., None]  #@shape g: [3, 1, 4, 1, 1]
    h = a[:, None]  #@shape h: [3, 1, 1, 4, 1]
    return b, c, d, e, f, g, h


def symbolic(x):
    B, T, C = x.shape
    u = x.unsqueeze(1)  #@shape u: [x.shape[0], 1, x.shape[1], x.shape[2]]
    v = u.squeeze(1)  #@shape v: [x.shape[0], x.shape[1], x.shape[2]]
    w = x.unsqueeze(-1).squeeze(-1)  #@shape w: [x.shape[0], x.shape[1], x.shape[2]]
    n = x[:, None]  #@shape n: [x.shape[0], 1, x.shape[1], x.shape[2]]
    # (was `[*, x.shape[2]]`: the join of [B, T, C] and [B, C] keeps both trailing positions,
    # unknown where they differ — also true, and more precise)
    y = x.squeeze(1)  #@shape y: [*, ?, x.shape[2]]
    r = y.dim()  #@dim r: ?
    return v, w, n, y


def made(n):
    a = torch.randn(1, n).squeeze(0)  #@shape a: [n]
    b = torch.randn(n, 1).squeeze(1)  #@shape b: [n]
    c = torch.randn(n, 1).squeeze(0)  #@shape c: [*, 1]
    return a, b, c


if __name__ == "__main__":
    literal()
    v, w, n, y = symbolic(torch.randn(2, 1, 4))  #@shape y: [2, 4]
    v2, w2, n2, y2 = symbolic(torch.randn(2, 3, 4))  #@shape y2: [2, 3, 4]
    made(1)
    made(5)

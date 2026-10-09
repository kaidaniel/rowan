# arithmetic on dims: B*T, C//n_head, 4*C, T+1, C-1 as ints and as sizes of new tensors
import torch


def arith(x, n_head):
    B, T, C = x.shape
    #@requires x.dim() == 3
    BT = B * T  #@dim BT: x.shape[0] * x.shape[1]
    hs = C // n_head  #@dim hs: x.shape[2] // n_head
    h = torch.zeros(B, T, 4 * C)  #@shape h: [x.shape[0], x.shape[1], 4*x.shape[2]]
    p = torch.zeros(B, T + 1, C)  #@shape p: [x.shape[0], x.shape[1] + 1, x.shape[2]]
    q = torch.zeros(B * T, C - 1)  #@shape q: [x.shape[0]*x.shape[1], x.shape[2] - 1]
    k = torch.zeros(B, n_head, T, hs)  #@shape k: [x.shape[0], n_head, x.shape[1], x.shape[2] // n_head]
    s = torch.zeros(2 * T - T, (C + C) - C)  #@shape s: [x.shape[1], x.shape[2]]
    return BT, hs, h, p, q, k, s


def concrete():
    d_model = 512
    n_head = 8
    hs = d_model // n_head  #@dim hs: 64
    t = torch.zeros(n_head, hs, 3 * hs + 1)  #@shape t: [8, 64, 193]
    return t


if __name__ == "__main__":
    arith(torch.randn(2, 3, 8), 4)
    concrete()

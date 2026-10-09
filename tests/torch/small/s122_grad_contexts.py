# shape ops inside `with torch.no_grad()` / autocast / inference_mode blocks and under @torch.no_grad()
import torch
import torch.nn as nn


def contexts(x):
    B, T, C = x.shape
    lin = nn.Linear(C, 2 * C)
    with torch.no_grad():
        y = lin(x)  #@shape y: [x.shape[0], x.shape[1], 2*x.shape[2]]
        #@requires none
    z = y.view(B, T, 2, C)  #@shape z: [x.shape[0], x.shape[1], 2, x.shape[2]]
    with torch.autocast("cpu", dtype=torch.bfloat16):
        m = z.mean(dim=2)  #@shape m: [x.shape[0], x.shape[1], x.shape[2]]
        a = lin(m)  #@shape a: [x.shape[0], x.shape[1], 2*x.shape[2]]
        #@requires none
    with torch.set_grad_enabled(False), torch.no_grad():
        b = a.transpose(1, 2)  #@shape b: [x.shape[0], 2*x.shape[2], x.shape[1]]
    with torch.inference_mode():
        c = b.sum(dim=-1)  #@shape c: [x.shape[0], 2*x.shape[2]]
    return c


@torch.no_grad()
def decorated(x):
    B, T = x.shape
    return x.unsqueeze(1).expand(B, 3, T)


@torch.inference_mode()
def decorated2(x, n):
    return x.repeat(n, 1)


def caller(x):
    B, T = x.shape
    e = decorated(x)  #@shape e: [x.shape[0], 3, x.shape[1]]
    r = decorated2(x, 2)  #@shape r: [2*x.shape[0], x.shape[1]]
    return e, r


if __name__ == "__main__":
    c = contexts(torch.randn(2, 3, 4))  #@shape c: [2, 8]
    e, r = caller(torch.randn(5, 6))  #@shape e: [5, 3, 6]; r: [10, 6]

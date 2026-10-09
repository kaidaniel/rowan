# helpers calling helpers three levels deep: dense -> ffn -> block, and an @L check from the driver
import torch
import torch.nn.functional as F


def dense(x, d_out):
    w = torch.randn(d_out, x.shape[-1])  #@shape w: [d_out, x.shape[-1]]
    y = F.linear(x, w)  #@shape y: [*x.shape[:-1], d_out]
    #@requires none
    #@shape@L34 y: [5, 7]
    return y


def ffn(x, hidden):
    B, T, C = x.shape
    #@requires x.dim() == 3
    h = torch.relu(dense(x, hidden))  #@shape h: [x.shape[0], x.shape[1], hidden]
    y = dense(h, C)  #@shape y: [x.shape[0], x.shape[1], x.shape[2]]
    return y


def block(x):
    B, T, C = x.shape
    #@requires x.dim() == 3
    inner = dense(x, 4 * C)  #@shape inner: [x.shape[0], x.shape[1], 4*x.shape[2]]
    y = x + ffn(x, 4 * C)  #@shape y: [x.shape[0], x.shape[1], x.shape[2]]
    #@requires none
    return y + inner.mean(-1, keepdim=True)


if __name__ == "__main__":
    y = block(torch.randn(2, 3, 4))  #@shape y: [2, 3, 4]
    z = ffn(torch.randn(1, 6, 8), 32)  #@shape z: [1, 6, 8]
    d = dense(torch.randn(5, 3), 7)  #@shape d: [5, 7]

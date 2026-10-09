# tensor methods and torch functions called with keyword arguments in unusual order bind each argument to the right parameter
import torch
import torch.nn.functional as F


def kw(x, w):
    B, T, C = x.shape
    a = x.transpose(dim1=2, dim0=1)  #@shape a: [x.shape[0], x.shape[2], x.shape[1]]
    b = torch.cat(dim=1, tensors=[x, x])  #@shape b: [x.shape[0], 2*x.shape[1], x.shape[2]]
    c = x.sum(keepdim=True, dim=2)  #@shape c: [x.shape[0], x.shape[1], 1]
    d = torch.transpose(x, 2, 0)  #@shape d: [x.shape[2], x.shape[1], x.shape[0]]
    e = x.permute(dims=(2, 0, 1))  #@shape e: [x.shape[2], x.shape[0], x.shape[1]]
    f = x.flatten(end_dim=1, start_dim=0)  #@shape f: [x.shape[0]*x.shape[1], x.shape[2]]
    g = torch.stack(dim=-1, tensors=[x, x])  #@shape g: [x.shape[0], x.shape[1], x.shape[2], 2]
    p = F.softmax(dim=1, input=x)  #@shape p: [x.shape[0], x.shape[1], x.shape[2]]
    r = x.repeat_interleave(repeats=2, dim=1)  #@shape r: [x.shape[0], 2*x.shape[1], x.shape[2]]
    m = torch.matmul(other=w, input=x)  #@shape m: [*broadcast([x.shape[0]], w.shape[:-2]), x.shape[1], w.shape[-1]] if w.dim() >= 2
    #@requires x.shape[2] == w.shape[-2] if w.dim() >= 2
    return a, b, c, d, e, f, g, p, r, m


if __name__ == "__main__":
    a, b, c, d, e, f, g, p, r, m = kw(torch.randn(2, 3, 4), torch.randn(4, 5))
    #@shape a: [2, 4, 3]; e: [4, 2, 3]; m: [2, 3, 5]

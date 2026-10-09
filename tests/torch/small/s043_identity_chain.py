# contiguous / clone / detach / dtype and device conversions keep the shape
import torch


def chain(x, y):
    a = x.contiguous()  #@shape a: [*x.shape]
    b = x.clone().detach()  #@shape b: [*x.shape]
    c = x.float().double().half()  #@shape c: [*x.shape]
    d = x.to(torch.float64)  #@shape d: [*x.shape]
    e = x.to("cpu")  #@shape e: [*x.shape]
    f = x.to(y)  #@shape f: [*x.shape]
    g = x.type_as(y).cpu()  #@shape g: [*x.shape]
    h = torch.clone(x).long().bool().int()  #@shape h: [*x.shape]
    #@requires none
    return a, b, c, d, e, f, g, h


def chain_known(x):
    B, T = x.shape  #@requires x.dim() == 2
    y = x.float().contiguous().view(B * T)  #@shape y: [x.shape[0]*x.shape[1]]
    z = x.detach().to(dtype=torch.float32, device="cpu")  #@shape z: [x.shape[0], x.shape[1]]
    return y, z


if __name__ == "__main__":
    chain(torch.randn(2, 3), torch.zeros(5, dtype=torch.float64))
    chain(torch.randn(4), torch.zeros(1))
    chain_known(torch.randn(3, 4))

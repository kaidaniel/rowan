# shape tuples as values: built from dims, concatenated with torch.Size, indexed, passed to factories
import torch


def build(x, n):
    B, T, C = x.shape
    #@requires x.dim() == 3
    shp = (B, T, 2 * C)  #@shape shp: [x.shape[0], x.shape[1], 2*x.shape[2]]
    a = torch.zeros(shp)  #@shape a: [x.shape[0], x.shape[1], 2*x.shape[2]]
    s2 = x.shape[:-1] + (n,)  #@shape s2: [x.shape[0], x.shape[1], n]
    b = x.new_zeros(s2)  #@shape b: [x.shape[0], x.shape[1], n]
    s3 = (1,) + x.shape[1:]  #@shape s3: [1, x.shape[1], x.shape[2]]
    c = torch.ones(*s3)  #@shape c: [1, x.shape[1], x.shape[2]]
    return a, b, c


def unknown_rank(x):
    s = x.shape
    a = torch.zeros(s[0], s[-1])  #@shape a: [x.shape[0], x.shape[-1]]
    s2 = (2,) + tuple(x.shape)  #@shape s2: [2, *x.shape]
    b = torch.zeros(s2)  #@shape b: [2, *x.shape]
    return a, b


if __name__ == "__main__":
    build(torch.randn(2, 3, 4), 5)
    unknown_rank(torch.randn(3, 4))
    unknown_rank(torch.randn(6))

# sizes from unpacking B, T, C = x.shape: the unpacking requires the rank, the dims are x.shape[i]
import torch


def build(x, y):
    B, T, C = x.shape  #@dim B: x.shape[0]; T: x.shape[1]; C: x.shape[2]
    #@requires x.dim() == 3
    a = torch.zeros(B, T)  #@shape a: [x.shape[0], x.shape[1]]
    b = torch.randn(T, C)  #@shape b: [x.shape[1], x.shape[2]]
    #@requires none
    n, d = y.shape  #@dim n: y.shape[0]; d: y.shape[1]
    #@requires y.dim() == 2
    c = torch.ones(n, C, d)  #@shape c: [y.shape[0], x.shape[2], y.shape[1]]
    _, T2, _ = x.shape  #@dim T2: x.shape[1]
    #@requires none
    k = x.size(-1)  #@dim k: x.shape[2]
    return a, b, c, k


if __name__ == "__main__":
    build(torch.randn(2, 3, 4), torch.randn(5, 6))

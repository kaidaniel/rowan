# loops whose body changes the shape every iteration (halving by ::2, doubling by cat): no wrong fixed point; literal trip counts are exact
import torch


def halve(x, n):
    B, T, C = x.shape
    y = x
    for _ in range(n):
        y = y[:, ::2]  #@shape y: [x.shape[0], ?, x.shape[2]]
    out = y  #@shape out: [x.shape[0], ?, x.shape[2]]
    return out


def double(x, n):
    B, T, C = x.shape
    y = x
    for _ in range(n):
        y = torch.cat([y, y], 1)  #@shape y: [x.shape[0], ?, x.shape[2]]
        #@requires none
    out = y  #@shape out: [x.shape[0], ?, x.shape[2]]
    return out


def literal_trips():
    y = torch.randn(2, 10, 4)
    for _ in range(3):
        y = y[:, ::2]
    a = y  #@shape a: [2, 2, 4]
    z = torch.randn(2, 3, 4)
    for _ in range(3):
        z = torch.cat([z, z], 1)
    b = z  #@shape b: [2, 24, 4]
    w = torch.randn(2, 10, 4)
    while w.shape[1] > 1:
        w = w[:, ::2]
    c = w  #@shape c: [2, 1, 4]
    return a, b, c


if __name__ == "__main__":
    h = halve(torch.randn(2, 10, 4), 2)  #@shape h: [2, 3, 4]
    d = double(torch.randn(2, 3, 4), 2)  #@shape d: [2, 12, 4]
    literal_trips()

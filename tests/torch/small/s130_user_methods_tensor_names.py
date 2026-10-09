# a user class with view / size / reshape / transpose / shape / __matmul__ is not a tensor
import torch
import torch.nn as nn


class Grid:
    def __init__(self, rows, cols):
        self.rows = rows
        self.cols = cols

    def view(self, *dims):
        return (self.rows, self.cols)

    def size(self, i=None):
        return self.rows * self.cols

    def reshape(self, n):
        return torch.zeros(n, self.rows)

    def transpose(self, a, b):
        return Grid(self.cols, self.rows)

    @property
    def shape(self):
        return (self.rows, self.cols)

    def __matmul__(self, other):
        return torch.zeros(self.rows, other.shape[-1])


def use(r, c):
    g = Grid(r, c)
    v = g.view(-1)  #@shape v: [r, c]
    n = g.size(0)  #@dim n: r*c
    t = g.reshape(5)  #@shape t: [5, r]
    g2 = g.transpose(0, 1)
    s = g2.shape  #@shape s: [c, r]
    z = torch.zeros(g2.shape)  #@shape z: [c, r]
    m = g @ torch.randn(4, 7)  #@shape m: [r, 7]
    return v, n, t, s, z, m


def call_view(obj):
    v = obj.view(-1)  #@shape v: ?
    return v


class Net(nn.Module):
    def __init__(self, d):
        super().__init__()
        self.d = d
        self.fc = nn.Linear(d, 2)

    def flatten(self, x):
        return x.mean(dim=1)

    def forward(self, x):
        B, T, C = x.shape
        h = self.flatten(x)  #@shape h: [x.shape[0], x.shape[2]]
        o = self.fc(h)  #@shape o: [x.shape[0], 2]
        #@requires x.shape[2] == self.d
        return o


if __name__ == "__main__":
    use(2, 3)
    v = call_view(Grid(2, 3))  #@shape v: [2, 3]
    w = call_view(torch.randn(2, 3))  #@shape w: [6]
    o = Net(4)(torch.randn(2, 5, 4))  #@shape o: [2, 2]

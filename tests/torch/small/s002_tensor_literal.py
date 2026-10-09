# torch.tensor of nested literal lists: rank and dims follow the list nesting; a scalar is 0-d
import torch


def literals():
    a = torch.tensor([[1, 2, 3], [4, 5, 6]])  #@shape a: [2, 3]
    b = torch.tensor(5)  #@shape b: []
    c = torch.tensor([1.0])  #@shape c: [1]
    d = torch.tensor([])  #@shape d: [0]
    e = torch.tensor([[[0], [1]], [[2], [3]], [[4], [5]]])  #@shape e: [3, 2, 1]
    f = torch.tensor([[1, 2]], dtype=torch.float32)  #@shape f: [1, 2]
    g = torch.tensor(2.5)  #@shape g: []
    return a, b, c, d, e, f, g


def used():
    w = torch.tensor([[0.5, -0.5], [1.0, 2.0], [3.0, 4.0]])  #@shape w: [3, 2]
    n = w.shape[0]  #@dim n: 3
    v = torch.tensor([[1, 2], [3, 4]]) + torch.tensor([10, 20])  #@shape v: [2, 2]
    return w, n, v


if __name__ == "__main__":
    literals()
    used()

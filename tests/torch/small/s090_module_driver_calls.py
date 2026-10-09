# a module instantiated in the driver and called with concrete tensors; @L checks inside forward
import torch
import torch.nn as nn
import torch.nn.functional as F


class Classifier(nn.Module):
    def __init__(self, in_dim, n_classes, hidden=64):
        super().__init__()
        self.net1 = nn.Linear(in_dim, hidden)
        self.net2 = nn.Linear(hidden, n_classes)

    def forward(self, x):
        h = F.relu(self.net1(x))  #@shape h: [*x.shape[:-1], hidden]
        #@requires x.shape[-1] == in_dim
        #@shape@L32 h: [32, 64]
        #@shape@L35 h: [3, 7, 16]
        y = self.net2(h)  #@shape y: [*x.shape[:-1], n_classes]
        #@requires none
        return y


def bad_input():
    lin = nn.Linear(20, 64)
    y = lin(torch.randn(32, 21))  #@shape-error
    return y


if __name__ == "__main__":
    model = Classifier(20, 10)
    x = torch.randn(32, 20)
    y = model(x)  #@shape y: [32, 10]
    pred = y.argmax(dim=-1)  #@shape pred: [32]
    loss = F.cross_entropy(y, torch.zeros(32, dtype=torch.long))  #@shape loss: []
    seq = Classifier(5, 2, hidden=16).forward(torch.randn(3, 7, 5))  #@shape seq: [3, 7, 2]
    try:
        bad_input()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

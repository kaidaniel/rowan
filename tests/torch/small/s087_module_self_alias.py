# user nn.Module: forward dims print through a self attribute (self.d_model) or the __init__ name (hidden)
import torch
import torch.nn as nn


class MLP(nn.Module):
    def __init__(self, d_model, hidden):
        super().__init__()
        self.d_model = d_model
        self.fc1 = nn.Linear(d_model, hidden)
        self.act = nn.GELU()
        self.fc2 = nn.Linear(hidden, d_model)

    def forward(self, x):
        h = self.fc1(x)  #@shape h: [*x.shape[:-1], hidden]
        #@requires x.shape[-1] == self.d_model
        h = self.act(h)  #@shape h: [*x.shape[:-1], hidden]
        #@requires none
        y = self.fc2(h)  #@shape y: [*x.shape[:-1], self.d_model]
        #@requires none
        z = x + y  #@shape z: [*x.shape[:-1], self.d_model]
        #@requires none
        return z


if __name__ == "__main__":
    m = MLP(16, 64)
    x = torch.randn(4, 7, 16)
    y = m(x)  #@shape y: [4, 7, 16]
    y1 = m(torch.randn(16))  #@shape y1: [16]
    h = m.fc1(x)  #@shape h: [4, 7, 64]

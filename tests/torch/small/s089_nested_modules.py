# a Block holding a user MLP and a LayerNorm: nested user modules, dims through self.d_model
import torch
import torch.nn as nn


class MLP(nn.Module):
    def __init__(self, d_model, hidden):
        super().__init__()
        self.d_model = d_model
        self.fc1 = nn.Linear(d_model, hidden)
        self.fc2 = nn.Linear(hidden, d_model)

    def forward(self, x):
        y = self.fc2(torch.relu(self.fc1(x)))  #@shape y: [*x.shape[:-1], self.d_model]
        #@requires x.shape[-1] == self.d_model
        return y


class Block(nn.Module):
    def __init__(self, d_model):
        super().__init__()
        self.d_model = d_model
        self.ln = nn.LayerNorm(d_model)
        self.mlp = MLP(d_model, 4 * d_model)

    def forward(self, x):
        h = self.ln(x)  #@shape h: [*x.shape[:-1], self.d_model]
        #@requires x.shape[-1] == self.d_model
        u = self.mlp.fc1(h)  #@shape u: [*x.shape[:-1], 4*self.d_model]
        #@requires none
        m = self.mlp(h)  #@shape m: [*x.shape[:-1], self.d_model]
        y = x + m  #@shape y: [*x.shape[:-1], self.d_model]
        #@requires none
        return y


if __name__ == "__main__":
    b = Block(8)
    y = b(torch.randn(2, 5, 8))  #@shape y: [2, 5, 8]
    z = b.mlp(torch.randn(3, 8))  #@shape z: [3, 8]

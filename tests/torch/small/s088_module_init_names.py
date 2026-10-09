# user nn.Module storing no dims: forward dims are written in __init__'s own argument names
import torch
import torch.nn as nn


class Head(nn.Module):
    def __init__(self, n_embd, head_size):
        super().__init__()
        self.key = nn.Linear(n_embd, head_size, bias=False)
        self.query = nn.Linear(n_embd, head_size, bias=False)
        self.value = nn.Linear(n_embd, head_size, bias=False)

    def forward(self, x):
        B, T, C = x.shape  #@dim B: x.shape[0]; T: x.shape[1]; C: x.shape[2]
        #@requires x.dim() == 3
        k = self.key(x)  #@shape k: [x.shape[0], x.shape[1], head_size]
        #@requires x.shape[2] == n_embd
        q = self.query(x)  #@shape q: [x.shape[0], x.shape[1], head_size]
        #@requires none
        wei = q @ k.transpose(-2, -1) * C ** -0.5  #@shape wei: [x.shape[0], x.shape[1], x.shape[1]]
        #@requires none
        wei = wei.softmax(dim=-1)
        v = self.value(x)
        out = wei @ v  #@shape out: [x.shape[0], x.shape[1], head_size]
        #@requires none
        return out


if __name__ == "__main__":
    head = Head(32, 8)
    out = head(torch.randn(2, 5, 32))  #@shape out: [2, 5, 8]

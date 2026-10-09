# subclasses overriding forward: super().forward(x) and Base.forward(self, x) then more layers
import torch
import torch.nn as nn


class Base(nn.Module):
    def __init__(self, d_in, d_out):
        super().__init__()
        self.d_out = d_out
        self.fc = nn.Linear(d_in, d_out)

    def forward(self, x):
        y = self.fc(x)  #@shape y: [*x.shape[:-1], self.d_out]
        #@requires x.shape[-1] == d_in
        return y


class WithHead(Base):
    def __init__(self, d_in, d_out, n_classes):
        super().__init__(d_in, d_out)
        self.n_classes = n_classes
        self.head = nn.Linear(d_out, n_classes)

    def forward(self, x):
        h = super().forward(x)  #@shape h: [*x.shape[:-1], self.d_out]
        y = self.head(h)  #@shape y: [*x.shape[:-1], self.n_classes]
        #@requires none
        return y


class Pooled(Base):
    def forward(self, x):
        B, T, C = x.shape
        h = Base.forward(self, x)  #@shape h: [x.shape[0], x.shape[1], self.d_out]
        p = h.mean(dim=1)  #@shape p: [x.shape[0], self.d_out]
        return p


if __name__ == "__main__":
    y = Base(4, 6)(torch.randn(3, 4))  #@shape y: [3, 6]
    z = WithHead(4, 6, 2)(torch.randn(5, 3, 4))  #@shape z: [5, 3, 2]
    p = Pooled(4, 6)(torch.randn(5, 3, 4))  #@shape p: [5, 6]

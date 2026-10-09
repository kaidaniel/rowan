# nn.Sequential stored as an attribute: called whole, indexed, and sliced
import torch
import torch.nn as nn


class Net(nn.Module):
    def __init__(self, d_in, d_hidden, d_out):
        super().__init__()
        self.d_out = d_out
        self.body = nn.Sequential(
            nn.Linear(d_in, d_hidden),
            nn.ReLU(),
            nn.Dropout(0.1),
            nn.Linear(d_hidden, d_out),
        )

    def forward(self, x):
        y = self.body(x)  #@shape y: [*x.shape[:-1], self.d_out]
        #@requires x.shape[-1] == d_in
        first = self.body[0](x)  #@shape first: [*x.shape[:-1], d_hidden]
        #@requires none
        tail = self.body[1:](first)  #@shape tail: [*x.shape[:-1], self.d_out]
        #@requires none
        return y + tail


if __name__ == "__main__":
    net = Net(10, 32, 4)
    y = net(torch.randn(8, 10))  #@shape y: [8, 4]
    z = net.body(torch.randn(2, 3, 10))  #@shape z: [2, 3, 4]

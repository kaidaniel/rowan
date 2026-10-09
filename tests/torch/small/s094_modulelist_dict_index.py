# ModuleList indexed (encoders[0], encoders[-1]) and ModuleDict looked up by literal key
import torch
import torch.nn as nn


class Heads(nn.Module):
    def __init__(self, d, n_classes, n_tags):
        super().__init__()
        self.d = d
        self.encoders = nn.ModuleList([nn.Linear(d, 2 * d), nn.Linear(2 * d, d)])
        self.heads = nn.ModuleDict({"cls": nn.Linear(d, n_classes), "tag": nn.Linear(d, n_tags)})

    def forward(self, x):
        h = self.encoders[0](x)  #@shape h: [*x.shape[:-1], 2*self.d]
        #@requires x.shape[-1] == self.d
        h = self.encoders[-1](h)  #@shape h: [*x.shape[:-1], self.d]
        #@requires none
        c = self.heads["cls"](h)  #@shape c: [*x.shape[:-1], n_classes]
        #@requires none
        t = self.heads["tag"](h)  #@shape t: [*x.shape[:-1], n_tags]
        #@requires none
        return c, t


if __name__ == "__main__":
    m = Heads(4, 3, 7)
    c, t = m(torch.randn(5, 4))  #@shape c: [5, 3]; t: [5, 7]
    e = m.encoders[1](torch.randn(2, 8))  #@shape e: [2, 4]

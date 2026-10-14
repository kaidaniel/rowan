# A layer object has every nn.Module method (`.train()`, `.eval()`, `.parameters()`) and its
# own attributes (`Dropout.p`, `Conv2d.bias`); rowan said "never set" for the ones its
# layer model does not keep (timm: `self.flatten.train(mode)`, `self.attn_drop.p`).
# Expectations written from the program text.
import torch
import torch.nn as nn


class Head(nn.Module):
    def __init__(self, c, drop=0.1):
        super().__init__()
        self.drop = nn.Dropout(drop)
        self.flatten = nn.Identity()
        self.fc = nn.Conv2d(c, 10, 1)

    def reset(self, mode):
        self.flatten.train(mode)               #@ sound train
        self.fc.eval()                         #@ sound eval
        return self.drop.p                     #@ sound p


h = Head(4)
p = h.reset(False)
x = h.fc(torch.zeros(2, 4, 5, 5))              #@shape x: [2, 10, 5, 5]

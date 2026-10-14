# A ModuleList / Sequential is a list and a ModuleDict a dict: `append`, `extend`, `insert`,
# `items()` are theirs. `ModuleList().append(Block(d))` left the list empty, so `ml[0](x)`
# raised (the rest of the program was never analysed) and a loop over it gave nothing.
# Expectations written from the program text.
import torch
import torch.nn as nn


class Block(nn.Module):
    def __init__(self, d):
        super().__init__()
        self.lin = nn.Linear(d, d)

    def forward(self, x):
        return self.lin(x)


ml = nn.ModuleList()
ml.append(Block(16))
x = torch.randn(4, 16)
y = ml[0](x)                                    #@shape y: [4, 16]
deep = nn.ModuleList()
for i in range(3):
    deep.append(Block(16))
h = x
for blk in deep:
    h = blk(h)
#@shape h: [4, 16]
md = nn.ModuleDict({"a": nn.Linear(16, 8), "b": nn.Linear(16, 4)})
outs = {}
for k, m in md.items():
    outs[k] = m(x)
oa = outs["a"]                                  #@shape oa: [4, 8]
ob = outs["b"]                                  #@shape ob: [4, 4]
n = len(deep)
#@value n: exactly={3}

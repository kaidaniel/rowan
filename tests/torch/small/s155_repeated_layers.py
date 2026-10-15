# A model of repeated layers (`nn.ModuleList([Block(d, h) for _ in range(L)])`, each block
# with its own list of heads) called in a training loop: unrolled, every forward ran L * h
# head forwards (12 blocks of 8 heads: 96), and a program calling the model a few dozen times
# ran its heads past their budget, losing the output's shape. The repeated layers are one
# summary each: a forward runs the block and the heads a few times. Shapes by hand: the heads
# are concatenated back to d features, the blocks keep [B, T, d].
#@work <= 400
import torch
from torch import nn


class Head(nn.Module):
    def __init__(self, d: int, dh: int) -> None:
        super().__init__()
        self.q = nn.Linear(d, dh, bias=False)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        return torch.relu(self.q(x))


class Block(nn.Module):
    def __init__(self, d: int, h: int) -> None:
        super().__init__()
        self.heads = nn.ModuleList([Head(d, d // h) for _ in range(h)])
        self.proj = nn.Linear(d, d)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        return x + self.proj(torch.cat([hd(x) for hd in self.heads], dim=-1))


class Net(nn.Module):
    def __init__(self, d: int, h: int, n_layers: int) -> None:
        super().__init__()
        self.blocks = nn.ModuleList([Block(d, h) for _ in range(n_layers)])
        self.out = nn.Linear(d, 3)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        for b in self.blocks:
            x = b(x)
        return self.out(x)


if __name__ == "__main__":
    net = Net(32, 8, 12)
    n = len(net.blocks)
    #@value n: exactly={12}
    k = len(net.blocks[0].heads)
    #@value k: exactly={8}
    for step in range(30):
        y = net(torch.randn(4, 10, 32))
        #@shape y: [4, 10, 3]
        for _ in range(5):
            yv = net(torch.randn(2, 10, 32))
            #@shape yv: [2, 10, 3]

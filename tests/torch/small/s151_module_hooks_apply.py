# `module.apply(fn)` calls fn on every submodule and then the module; a forward hook runs at
# each call of its module (`hook(module, args, output)`, a result other than None replaces the
# output). Both were taken to do nothing: `count` and `calls` stayed empty.
# `type(m)` of a ModuleList is ModuleList, not list. Expectations written from the program text.
import torch
import torch.nn as nn


class Stack(nn.Module):
    def __init__(self, d, n):
        super().__init__()
        self.blocks = nn.ModuleList([nn.Linear(d, d) for _ in range(n)])
        self.head = nn.Linear(d, 2)

    def forward(self, x):
        for blk in self.blocks:
            x = blk(x)
        return self.head(x)


calls = []


def hook(mod, inp, out):
    calls.append(out.shape[-1])


def double(mod, inp, out):
    return torch.cat([out, out], dim=-1)


if __name__ == "__main__":
    s = Stack(16, 3)
    count = []
    s.apply(lambda m: count.append(type(m).__name__))
    nm = len(count)
    #@value nm: includes={6}
    last = count[-1]
    #@value last: includes={"Stack"}
    s.head.register_forward_hook(hook)
    x = torch.randn(4, 16)
    y = s(x)                                    #@shape y: [4, 2]
    k = len(calls)
    #@value k: includes={1}
    s.head.register_forward_hook(double)
    y2 = s(x)                                   #@shape y2: [4, _]
    #@todo shape y2: [4, 4] (a hook runs as perhaps removed: no handle tracks it)

# layer sizes from a config dict and an argparse-like namespace: dims are named cfg["hidden"] / args.hidden; a literal dict gives literal dims
import types

import torch
import torch.nn as nn


class MLP(nn.Module):
    def __init__(self, cfg):
        super().__init__()
        self.cfg = cfg
        self.fc1 = nn.Linear(cfg["d_model"], cfg["hidden"])
        self.fc2 = nn.Linear(cfg["hidden"], cfg["d_model"])

    def forward(self, x):
        h = self.fc1(x)  #@shape h: [*x.shape[:-1], self.cfg["hidden"]]
        #@requires x.shape[-1] == self.cfg["d_model"]
        y = self.fc2(torch.relu(h))  #@shape y: [*x.shape[:-1], self.cfg["d_model"]]
        #@requires none
        return y


def build(args):
    lin = nn.Linear(args.dim, args.hidden)
    x = torch.randn(args.batch, args.dim)  #@shape x: [args.batch, args.dim]
    y = lin(x)  #@shape y: [args.batch, args.hidden]
    #@requires none
    z = y.view(args.batch, args.heads, -1)  #@shape z: [args.batch, args.heads, _]
    return z


def literal_cfg():
    m = MLP({"d_model": 16, "hidden": 32})
    h = m.fc1(torch.randn(2, 16))  #@shape h: [2, 32]
    y = m(torch.randn(3, 5, 16))  #@shape y: [3, 5, 16]
    return h, y


def literal_bad():
    m = MLP({"d_model": 16, "hidden": 32})
    h = m.fc1(torch.randn(2, 32))  #@shape-error
    return h


if __name__ == "__main__":
    m = MLP({"d_model": 8, "hidden": 24})
    y = m(torch.randn(4, 8))  #@shape y: [4, 8]
    z = build(types.SimpleNamespace(dim=6, hidden=12, batch=3, heads=4))  #@shape z: [3, 4, 3]
    literal_cfg()
    try:
        literal_bad()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

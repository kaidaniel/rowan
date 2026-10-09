# torch.Tensor annotations and isinstance(x, torch.Tensor) decide the branch (nn.Parameter is a Tensor)
from typing import List

import torch
import torch.nn as nn


def typed(x: torch.Tensor, n: int) -> torch.Tensor:
    B, T = x.shape
    y = x.repeat(1, n)  #@shape y: [x.shape[0], n*x.shape[1]]
    return y


def maybe_tensor(x):
    if isinstance(x, torch.Tensor):
        y = x.unsqueeze(-1)  #@shape y: [*x.shape, 1]
    else:
        y = torch.tensor(x)
    return y


def stack_any(x):
    if isinstance(x, torch.Tensor):
        x = [x]
    s = torch.stack(x)
    return s


def typed_list(xs: List[torch.Tensor]) -> torch.Tensor:
    return torch.stack(xs, dim=1)


def caller(B, T):
    t = torch.randn(B, T)
    s = typed_list([t, t, t])  #@shape s: [B, 3, T]
    a = stack_any(t)  #@shape a: [1, B, T]
    b = stack_any([t, t])  #@shape b: [2, B, T]
    m = maybe_tensor(t)  #@shape m: [B, T, 1]
    l = maybe_tensor([1.0, 2.0])  #@shape l: [2]
    p = maybe_tensor(nn.Parameter(torch.zeros(3, 4)))  #@shape p: [3, 4, 1]
    return s, a, b, m, l, p


if __name__ == "__main__":
    y = typed(torch.randn(2, 3), 4)  #@shape y: [2, 12]
    caller(5, 6)

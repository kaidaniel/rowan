# tensors as dataclass fields, and tensors from json / unknown calls: no shape may be claimed for those
import json
from dataclasses import dataclass

import torch
import torch.nn as nn


@dataclass
class Batch:
    ids: torch.Tensor
    mask: torch.Tensor


def make_batch(B, T):
    b = Batch(torch.zeros(B, T, dtype=torch.long), torch.ones(B, T))
    ids = b.ids  #@shape ids: [B, T]
    return b


def embed(b):
    e = nn.Embedding(100, 8)(b.ids)  #@shape e: [*b.ids.shape, 8]
    #@requires none
    return e


def from_json(text):
    data = json.loads(text)
    t = torch.tensor(data["x"])  #@shape t: ?
    return t


def opaque(load):
    w = load("w.pt")  #@shape w: ?
    y = w * 2  #@shape y: [*w.shape]
    return y


if __name__ == "__main__":
    b = make_batch(2, 5)
    e = embed(b)  #@shape e: [2, 5, 8]
    t = from_json('{"x": [[1.0, 2.0, 3.0]]}')  #@shape t: [1, 3]
    loader = lambda path: torch.randn(3, 4)
    y = opaque(loader)  #@shape y: [3, 4]

# view(n, -1) where n and the tensor's dims are unknown (a tensor of parsed data, rank unknown): two unknown dims are not one dim, so -1 cancels none of them (L22: precise [len(s), 6], but rowan does not see the two len(s) are one dim)
import json
import torch
import torch.nn as nn


def tag(text):
    s = torch.tensor(json.loads(text))
    emb = nn.Embedding(10, 6)
    e = emb(s)
    v = e.view(len(s), 1, -1)  #@shape v: [_, 1, _]
    lstm = nn.LSTM(6, 6)
    lo, (h, c) = lstm(v)
    w = lo.view(len(s), -1)  #@shape w: [_, _]
    out = nn.Linear(6, 3)(w)  #@shape out: [_, 3]
    return out


def flat(text, m):
    s = torch.tensor(json.loads(text))
    a = torch.zeros(len(s), 1, 6)
    w = a.view(len(s), -1)  #@shape w: [_, ?]
    b = torch.zeros(m, 1, 6)
    w3 = b.view(len(s), -1)  #@shape w3: [_, ?]
    return w, w3


if __name__ == "__main__":
    o = tag("[1, 2, 3, 4]")  #@shape o: [4, 3]
    w, w3 = flat("[1, 2]", 2)  #@shape w: [2, 6]; w3: [2, 6]

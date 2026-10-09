# tensors stored in dicts and tuples: out["logits"], pair[1]; an unknown model's outputs stay unknown
import torch
import torch.nn as nn


class Enc(nn.Module):
    def __init__(self, d, n_classes):
        super().__init__()
        self.d = d
        self.n_classes = n_classes
        self.body = nn.Linear(d, d)
        self.head = nn.Linear(d, n_classes)

    def forward(self, x):
        B, T, C = x.shape
        h = self.body(x)
        out = {"hidden": h, "logits": self.head(h)}
        logits = out["logits"]  #@shape logits: [x.shape[0], x.shape[1], self.n_classes]
        hidden = out["hidden"]  #@shape hidden: [x.shape[0], x.shape[1], self.d]
        pair = (h, h.mean(dim=1))
        m = pair[1]  #@shape m: [x.shape[0], self.d]
        out["pooled"] = m
        return out


def consume(model, x):
    out = model(x)
    logits = out["logits"]  #@shape logits: ?
    return logits


if __name__ == "__main__":
    enc = Enc(4, 3)
    res = enc(torch.randn(2, 5, 4))
    lg = res["logits"]  #@shape lg: [2, 5, 3]
    pooled = res["pooled"]  #@shape pooled: [2, 4]
    lg2 = consume(enc, torch.randn(1, 2, 4))  #@shape lg2: [1, 2, 3]

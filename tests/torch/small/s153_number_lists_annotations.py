# Tensors made from lists rowan has no length for, and parameters' annotations
# (pytorch-playground's tokenisers and attention): a list of numbers is one dim
# (`torch.tensor([idx[c] for c in s])`, with `idx` an index the program built from unknown
# text: a key it has holds one of the ints it stored); a comprehension written in the call
# over a known str or a tensor has its length; `set` of a known string holds its distinct
# characters; a parameter annotated `int` is a number (`x / d ** 0.5` is x's shape, not a
# broadcast with d), one annotated `Tensor` is indexed as a tensor; a product of tensors of
# unknown ranks times a matrix ends in the matrix's columns; `randn(*dims)` spreads a sequence
# of sizes (it was taken as one size, `[dims]`: rank 1, unsound).
import os

import torch

text = os.environ.get("ROWAN_TEXT", "the cat sat on the mat")
vocab = sorted(set(text))
idx = {c: i for i, c in enumerate(vocab)}


def encode(s: str):
    out = torch.tensor([idx[c] for c in s])
    #@shape out: [len(s)]
    # (a comprehension over the str s: one item per character)
    return out


def scale(x: torch.Tensor, d: int):
    s = x / d ** 0.5
    #@shape s: [*x.shape]
    return s


def windows(data: torch.Tensor, starts: torch.Tensor, T: int):
    w = torch.stack([data[j : j + T] for j in starts])
    #@shape w: [starts.shape[0], _, *data.shape[1:]]
    return w


def readout(a, b, dims):
    o = (a @ b) @ torch.ones(4, 3)
    #@shape o: [*broadcast(a.shape[:-2], b.shape[:-2]), a.shape[-2], 3] if a.dim() >= 2, b.dim() >= 2
    a = torch.randn(*dims)
    #@shape a: [*]
    r = a @ torch.ones(4, 3)
    #@shape r: [*_, 3]
    return o, r


lit = "abracadabra"
chars = sorted(set(lit))
n = len(chars)
#@value n: exactly={5}
ids = torch.tensor([ord(c) for c in lit])
#@shape ids: [11]
rows = torch.stack([ids[i : i + 2] for i in range(3)])
#@shape rows: [3, 2]

if __name__ == "__main__":
    e = encode("cat")
    #@shape e: [3]
    s = scale(torch.randn(2, 5), 4)
    #@shape s: [2, 5]
    o, r = readout(torch.randn(2, 5), torch.randn(5, 4), (6, 4))
    #@shape o: [2, 3]; r: [6, 3]
    w = windows(ids, torch.tensor([0, 2, 4, 6]), 3)
    #@shape w: [4, _]
    #@todo shape w: [4, 3] (a slice j:j + T of unknown starts is T long only when j + T <= len(data))

# A number the module computes from data rowan cannot see (`V = len(vocab)` of an unknown
# text, pytorch-playground's rnn-model.py) names the dims that equal it: `Linear(d, V)` gives
# `[..., V]`, not `[..., ?]`. Only a name the module binds once outside loops: a name bound
# again, or in a loop, keeps its value unnamed (one name, one value). And `text.split()` of a
# text that is the default or unknown is not the default's split: `n` was 2 (unsound).
import os

import torch
import torch.nn as nn

text = os.environ.get("ROWAN_TEXT", "hello world")
vocab = sorted(set(text))
V = len(vocab)
head = nn.Linear(4, V)
y = head(torch.randn(2, 4))
#@shape y: [2, V]
w = torch.zeros(V + 1, 2)
#@shape w: [V + 1, 2]
n = len(text)
n = len(text.split())
x = torch.zeros(n)
#@shape x: [?]
for word in text.split():
    k = len(word)
    t = torch.ones(k)
    #@shape t: [?]

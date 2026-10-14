# keepdim / negative-dim reductions of a shape with known leading dims and an unknown rest
# (a prompt of unknown length made 2-D): the reduced last dim may be one of the leading dims
# (the rest may be empty), so the result has no more dims than the input. Before, `[1, ?, *]`
# reduced on dim -1 became `[1, ?, *, 1]`, and the embedding of it "needed 3 dims, got at
# least 4": a false mismatch (pytorch-playground rnn-model.py, greedy generation).
import os

import torch
import torch.nn as nn

text = os.environ.get("ROWAN_PROMPT", "hello")
ids = torch.tensor([ord(c) for c in text])[None, :]
#@shape ids: [1, _, *_]
last = ids.argmax(dim=-1, keepdim=True)
#@shape last: [*_, 1]
total = ids.sum(-1)
#@shape total: [*_]
e = nn.Embedding(128, 4)(last)
#@shape e: [*_, 1, 4]
B, T, C = e.shape
#@dim C: 4

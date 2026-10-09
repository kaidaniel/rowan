# a dataclass Config passed to __init__: dims as self.config.n_embd (stored) or config.n_embd (not stored)
from dataclasses import dataclass

import torch
import torch.nn as nn


@dataclass
class Config:
    vocab_size: int = 100
    block_size: int = 16
    n_embd: int = 32
    n_head: int = 4


class Model(nn.Module):
    def __init__(self, config):
        super().__init__()
        self.config = config
        self.wte = nn.Embedding(config.vocab_size, config.n_embd)
        self.wpe = nn.Embedding(config.block_size, config.n_embd)
        self.lm_head = nn.Linear(config.n_embd, config.vocab_size, bias=False)

    def forward(self, idx):
        B, T = idx.shape
        #@requires idx.dim() == 2
        pos = torch.arange(0, T)  #@shape pos: [idx.shape[1]]
        tok = self.wte(idx)  #@shape tok: [idx.shape[0], idx.shape[1], self.config.n_embd]
        pe = self.wpe(pos)  #@shape pe: [idx.shape[1], self.config.n_embd]
        x = tok + pe  #@shape x: [idx.shape[0], idx.shape[1], self.config.n_embd]
        logits = self.lm_head(x)  #@shape logits: [idx.shape[0], idx.shape[1], self.config.vocab_size]
        #@requires none
        return logits


class HeadProj(nn.Module):
    def __init__(self, config):
        super().__init__()
        self.proj = nn.Linear(config.n_embd, config.n_embd // config.n_head)

    def forward(self, x):
        y = self.proj(x)  #@shape y: [*x.shape[:-1], config.n_embd // config.n_head]
        #@requires x.shape[-1] == config.n_embd
        return y


if __name__ == "__main__":
    cfg = Config()
    logits = Model(cfg)(torch.randint(0, 100, (2, 9)))  #@shape logits: [2, 9, 100]
    y = HeadProj(Config(n_embd=12, n_head=3))(torch.randn(5, 12))  #@shape y: [5, 4]

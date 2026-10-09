# staticmethod and classmethod helpers on a module: positions(n), add_pos(x, pe), Tok.small()
import torch
import torch.nn as nn


class Tok(nn.Module):
    def __init__(self, vocab, dim):
        super().__init__()
        self.dim = dim
        self.emb = nn.Embedding(vocab, dim)

    @classmethod
    def small(cls):
        return cls(100, 8)

    @staticmethod
    def positions(n):
        p = torch.arange(n)  #@shape p: [n]
        return p

    @staticmethod
    def add_pos(x, pe):
        y = x + pe  #@shape y: [*broadcast(x.shape, pe.shape)]
        return y

    def forward(self, ids):
        B, T = ids.shape
        #@requires ids.dim() == 2
        pos = self.positions(T)  #@shape pos: [ids.shape[1]]
        e = self.emb(ids)  #@shape e: [ids.shape[0], ids.shape[1], self.dim]
        #@requires none
        pe = self.emb(pos)  #@shape pe: [ids.shape[1], self.dim]
        y = self.add_pos(e, pe)  #@shape y: [ids.shape[0], ids.shape[1], self.dim]
        return y


if __name__ == "__main__":
    m = Tok.small()
    y = m(torch.randint(0, 100, (2, 5)))  #@shape y: [2, 5, 8]
    p = Tok.positions(9)  #@shape p: [9]

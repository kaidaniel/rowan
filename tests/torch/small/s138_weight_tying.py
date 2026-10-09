# weight tying (head.weight = emb.weight), one layer object under two names, and a Linear whose weight is replaced by one of another shape
import torch
import torch.nn as nn
import torch.nn.functional as F


class LM(nn.Module):
    def __init__(self, vocab, d):
        super().__init__()
        self.vocab = vocab
        self.d = d
        self.emb = nn.Embedding(vocab, d)
        self.head = nn.Linear(d, vocab, bias=False)
        self.head.weight = self.emb.weight
        self.proj = nn.Linear(d, d)
        self.proj_out = self.proj
        self.retarget = nn.Linear(d, 2 * d, bias=False)
        self.retarget.weight = nn.Parameter(torch.randn(vocab, d))

    def forward(self, idx):
        B, T = idx.shape
        h = self.emb(idx)  #@shape h: [idx.shape[0], idx.shape[1], self.d]
        g = self.proj_out(self.proj(h))  #@shape g: [idx.shape[0], idx.shape[1], self.d]
        #@requires none
        logits = self.head(g)  #@shape logits: [idx.shape[0], idx.shape[1], self.vocab]
        #@requires none
        w = self.head.weight  #@shape w: [self.vocab, self.d]
        lt = g @ self.emb.weight.T  #@shape lt: [idx.shape[0], idx.shape[1], self.vocab]
        #@requires none
        lf = F.linear(g, self.head.weight)  #@shape lf: [idx.shape[0], idx.shape[1], self.vocab]
        #@requires none
        r = self.retarget(g)  #@shape r: [idx.shape[0], idx.shape[1], self.vocab]
        #@requires none
        return logits, r


if __name__ == "__main__":
    lm = LM(11, 6)
    logits, r = lm(torch.randint(0, 11, (2, 5)))  #@shape logits: [2, 5, 11]; r: [2, 5, 11]

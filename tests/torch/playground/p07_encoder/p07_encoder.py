# The owner's pytorch-playground `pytorch_playground/encoder.py`, analysed as the main module
# (as the editor does when it is open; Block comes from pytorch_playground/transformers.py),
# unchanged but for this header, the directives and a driver (the owner's has none): a
# bidirectional transformer encoder and three heads on it. Checked in forward()'s own terms
# (open `ids`; `self` built by `__init__` from open arguments), and in the driver: an encoder
# of 2 blocks over a vocabulary of 50 on 3 sequences of 10 ids; the heads' outputs are the
# owner's comments (B, T, V), (B, T, K), (B, K). A causal block runs the masked branch.
import torch
from torch import nn, Tensor
from pytorch_playground.transformers import Block

class Encoder(nn.Module):
    def __init__(self, d: int, V: int, context_length: int, n_heads: int, n_layers: int) -> None:
        super().__init__()
        self.d = d
        self.context_length = context_length
        self.tok_emb = nn.Embedding(V, d)
        self.pos_emb = nn.Embedding(context_length, d)
        self.blocks = nn.ModuleList(
            [Block(d, n_heads, context_length, causal=False) for _ in range(n_layers)]
        )
        self.ln_f = nn.LayerNorm(d)

    def forward(self, ids: Tensor) -> Tensor:
        # ids: (B, T) int
        _ , T = ids.shape
        #@dim T: ids.shape[1]
        #@requires ids.dim() == 2
        x = self.tok_emb(ids) # (B, T, d)
        #@shape x: [ids.shape[0], ids.shape[1], self.d]
        p = self.pos_emb(torch.arange(T, device=ids.device)) # (T, d)
        #@shape p: [ids.shape[1], self.d]
        x = x + p # (B, T, d) - p is broadcast over B.
        #@shape x: [ids.shape[0], ids.shape[1], self.d]
        for block in self.blocks:
            x = block(x) # (B, T, d)
            #@shape x: [ids.shape[0], ids.shape[1], self.d]
        return self.ln_f(x) # (B, T, d)

class MaskedLM(nn.Module):                              # pretraining wrapper
    def __init__(self, enc: Encoder) -> None:
        super().__init__()
        self.enc = enc
        self.head = nn.Linear(enc.tok_emb.embedding_dim, enc.tok_emb.num_embeddings)
    def forward(self, ids: Tensor) -> Tensor:           # (B, T, V)
        return self.head(self.enc(ids))

class TokenClassifier(nn.Module):                       # e.g. POS / BMES tagging
    def __init__(self, enc: Encoder, K: int) -> None:
        super().__init__()
        self.enc = enc
        self.head = nn.Linear(enc.tok_emb.embedding_dim, K)
    def forward(self, ids: Tensor) -> Tensor:           # (B, T, K)
        return self.head(self.enc(ids))

class SequenceClassifier(nn.Module):                    # reads [CLS] = position 0
    def __init__(self, enc: Encoder, K: int) -> None:
        super().__init__()
        self.enc = enc
        self.head = nn.Linear(enc.tok_emb.embedding_dim, K)
    def forward(self, ids: Tensor) -> Tensor:           # (B, K)
        return self.head(self.enc(ids)[:, 0])


if __name__ == "__main__":
    enc = Encoder(d=32, V=50, context_length=16, n_heads=4, n_layers=2)
    ids = torch.randint(0, 50, (3, 10))
    #@shape ids: [3, 10]
    h = enc(ids)
    #@shape h: [3, 10, 32]
    logits = MaskedLM(enc)(ids)
    #@shape logits: [3, 10, 50]
    tags = TokenClassifier(enc, K=5)(ids)
    #@shape tags: [3, 10, 5]
    cls = SequenceClassifier(enc, K=2)(ids)
    #@shape cls: [3, 2]
    blk = Block(32, 4, 16)
    out = blk(torch.randn(3, 10, 32))
    #@shape out: [3, 10, 32]

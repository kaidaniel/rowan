# The owner's pytorch-playground `pytorch_playground/transformers.py`, unchanged but for this
# header and one check, as the module the case's entry imports (its shapes in its own terms
# are checked in p08_transformers.py, where it is the main module).
from torch import Tensor, nn
import torch

class Head(nn.Module):
    def __init__(self, d_model: int, d_head: int, block_size: int, causal: bool = True) -> None:
        super().__init__()
        self.d_head = d_head
        self.causal = causal
        self.Q = nn.Linear(d_model, d_head, bias=False)
        self.K = nn.Linear(d_model, d_head, bias=False)
        self.V = nn.Linear(d_model, d_head, bias=False)
        self.tril: Tensor
        if self.causal:
            tril = torch.tril(torch.ones(block_size, block_size, dtype=torch.bool))
            self.register_buffer("tril", tril)

    def forward(self, x: Tensor) -> Tensor:
        # x: (B, T, d_model)
        # out: (B, T, d_head)
        B, T, d = x.shape
        Qx: Tensor = self.Q(x) # (B, T, d_head) "what I attend to"
        Kx: Tensor = self.K(x) # (B, T, d_head) "what can attend to me"
        Vx: Tensor = self.V(x) # (B, T, d_head) "what I deliver to those attending"
        KxT: Tensor = Kx.transpose(-2, -1) # (B, d_head, T)
        S: Tensor = Qx @ KxT / self.d_head ** 0.5 # (B, T, T) "pairwise attention-scores of positions". Rowan doesn't seem to use the fact that self.d_head is an int and treats it as "broadcast".
        if self.causal:
            S = S.masked_fill(~self.tril[:T, :T], float("-inf"))
            #@ ok tril
        A = torch.softmax(S, dim=-1) # (B, T, T) such that last dim sums to 1 => "pairwise attentions of positions". S is "Tensor[*]" here when we actually _do_ know it's shape.
        out = A @ Vx # (B, T, d_head) "what I collect after attending"
        return out # out is Tensor[*] here but it's shape is actually quite easy to express in terms of B, T, d?

class MultiHead(nn.Module):
    def __init__(self, d_model: int, n_heads: int, block_size: int, causal: bool = True) -> None:
        super().__init__()
        assert d_model % n_heads == 0
        d_head = d_model // n_heads
        self.heads = nn.ModuleList([Head(d_model, d_head, block_size, causal) for _ in range(n_heads)])
        self.proj = nn.Linear(d_model, d_model)

    def forward(self, x: Tensor) -> Tensor:
        # x: (B, T, d_model)
        return self.proj(torch.cat([h(x) for h in self.heads], dim=-1)) # (B, T, d_model)


class Block(nn.Module):
    def __init__(self, d_model: int, n_heads: int, block_size: int, causal: bool = True) -> None:
        super().__init__()
        self.ln1 = nn.LayerNorm(d_model)
        self.attn = MultiHead(d_model, n_heads, block_size, causal)
        self.ln2 = nn.LayerNorm(d_model)
        self.mlp = nn.Sequential( # the MLP
            nn.Linear(d_model, 4 * d_model),  # 4x dimensions because they act as feature detectors with GELU.
            nn.GELU(),
            nn.Linear(4 * d_model, d_model)
        ) # (d_model, d_model)

    def forward(self, x: Tensor) -> Tensor:
        # x: (B, T, d_model)
        # out: (B, T, d_model)
        x = x + self.attn(self.ln1(x))
        x = x + self.mlp(self.ln2(x))
        return x

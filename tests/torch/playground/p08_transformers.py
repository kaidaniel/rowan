# The owner's pytorch-playground `pytorch_playground/transformers.py`, analysed as the main
# module (as the editor does when it is open), unchanged but for this header, the directives
# and a driver: one attention head, multi-head attention as a list of heads, a pre-LayerNorm
# transformer block. Checked in each method's own terms (open arguments; `self` built by
# `__init__` from open arguments): the owner's shape comments. `self.d_head ** 0.5` is a
# number, so `S` is [B, T, T]; the causal mask [:T, :T] keeps it.
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
            #@shape tril: [block_size, block_size]
            self.register_buffer("tril", tril)

    def forward(self, x: Tensor) -> Tensor:
        # x: (B, T, d_model)
        # out: (B, T, d_head)
        B, T, d = x.shape
        #@dim B: x.shape[0]; T: x.shape[1]; d: x.shape[2]
        #@requires x.dim() == 3
        Qx: Tensor = self.Q(x) # (B, T, d_head) "what I attend to"
        #@shape Qx: [x.shape[0], x.shape[1], self.d_head]
        #@requires x.shape[2] == d_model
        Kx: Tensor = self.K(x) # (B, T, d_head) "what can attend to me"
        #@shape Kx: [x.shape[0], x.shape[1], self.d_head]
        Vx: Tensor = self.V(x) # (B, T, d_head) "what I deliver to those attending"
        #@shape Vx: [x.shape[0], x.shape[1], self.d_head]
        KxT: Tensor = Kx.transpose(-2, -1) # (B, d_head, T)
        #@shape KxT: [x.shape[0], self.d_head, x.shape[1]]
        S: Tensor = Qx @ KxT / self.d_head ** 0.5 # (B, T, T) "pairwise attention-scores of positions". Rowan doesn't seem to use the fact that self.d_head is an int and treats it as "broadcast".
        #@shape S: [x.shape[0], x.shape[1], x.shape[1]]
        if self.causal:
            S = S.masked_fill(~self.tril[:T, :T], float("-inf"))
            #@shape S: [x.shape[0], x.shape[1], x.shape[1]]
            #@ ok tril
        A = torch.softmax(S, dim=-1) # (B, T, T) such that last dim sums to 1 => "pairwise attentions of positions". S is "Tensor[*]" here when we actually _do_ know it's shape.
        #@shape A: [x.shape[0], x.shape[1], x.shape[1]]
        out = A @ Vx # (B, T, d_head) "what I collect after attending"
        #@shape out: [x.shape[0], x.shape[1], self.d_head]
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
        #@shape x: [*_, d_model]
        #@todo shape x: [x.shape[0], x.shape[1], d_model] (the heads need x.dim() == 3, but that fact is lost after MultiHead's loop over a list of heads of unknown length: rowan does not use that cat of no tensors raises)
        x = x + self.mlp(self.ln2(x))
        #@shape x: [*_, d_model]
        return x


if __name__ == "__main__":
    h = Head(32, 8, 16)
    o = h(torch.randn(3, 10, 32))
    #@shape o: [3, 10, 8]
    mh = MultiHead(32, 4, 16, causal=False)
    o2 = mh(torch.randn(3, 10, 32))
    #@shape o2: [3, 10, 32]
    blk = Block(32, 4, 16)
    o3 = blk(torch.randn(3, 10, 32))
    #@shape o3: [3, 10, 32]

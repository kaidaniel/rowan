# nanoGPT (Karpathy, github.com/karpathy/nanoGPT, model.py; GPT-2, Radford et al. 2019),
# written from scratch after it: GPTConfig dataclass, LayerNorm with optional bias,
# CausalSelfAttention with one c_attn Linear split into q/k/v, the causal `tril` buffer
# [1, 1, block, block] sliced [:, :, :T, :T], MLP, Block, GPT with wte/wpe in a ModuleDict,
# ln_f, lm_head (weight-tied), forward(idx, targets=None) -> (logits, loss), generate.
# Checked: config dims through `config.*` / `self.config.*` / `self.n_embd` (and the shortest
# self path `self.attn.n_embd` in Block), the split into three [B, T, C] tensors, head
# view/transpose, the flash and the manual attention paths, the loss on flattened views, the
# last-position logits, the join of the two forward branches; concrete shapes in the driver;
# one c_attn call with the wrong embedding width.
import math
from dataclasses import dataclass

import torch
import torch.nn as nn
from torch.nn import functional as F


@dataclass
class GPTConfig:
    block_size: int = 1024
    vocab_size: int = 50304
    n_layer: int = 12
    n_head: int = 12
    n_embd: int = 768
    dropout: float = 0.0
    bias: bool = True


class LayerNorm(nn.Module):
    """LayerNorm with an optional bias (PyTorch's has no bias=False before 2.1)."""

    def __init__(self, ndim, bias):
        super().__init__()
        self.weight = nn.Parameter(torch.ones(ndim))
        self.bias = nn.Parameter(torch.zeros(ndim)) if bias else None

    def forward(self, input):
        out = F.layer_norm(input, self.weight.shape, self.weight, self.bias, 1e-5)
        #@shape out: [*input.shape[:-1], ndim]
        #@requires input.shape[-1] == ndim
        return out


class CausalSelfAttention(nn.Module):
    def __init__(self, config):
        super().__init__()
        assert config.n_embd % config.n_head == 0
        self.c_attn = nn.Linear(config.n_embd, 3 * config.n_embd, bias=config.bias)
        self.c_proj = nn.Linear(config.n_embd, config.n_embd, bias=config.bias)
        self.attn_dropout = nn.Dropout(config.dropout)
        self.resid_dropout = nn.Dropout(config.dropout)
        self.n_head = config.n_head
        self.n_embd = config.n_embd
        self.dropout = config.dropout
        self.flash = hasattr(torch.nn.functional, "scaled_dot_product_attention")
        mask = torch.tril(torch.ones(config.block_size, config.block_size)).view(1, 1, config.block_size, config.block_size)
        #@shape mask: [1, 1, config.block_size, config.block_size]
        self.register_buffer("bias", mask)

    def forward(self, x):
        B, T, C = x.size()  #@dim B: x.shape[0]; T: x.shape[1]; C: x.shape[2]
        #@requires x.dim() == 3
        qkv = self.c_attn(x)  #@shape qkv: [x.shape[0], x.shape[1], 3*self.n_embd]
        #@requires x.shape[2] == self.n_embd
        q, k, v = qkv.split(self.n_embd, dim=2)  #@shape q: [x.shape[0], x.shape[1], self.n_embd]; v: [x.shape[0], x.shape[1], self.n_embd]
        k = k.view(B, T, self.n_head, C // self.n_head).transpose(1, 2)
        #@shape k: [x.shape[0], self.n_head, x.shape[1], self.n_embd // self.n_head]
        q = q.view(B, T, self.n_head, C // self.n_head).transpose(1, 2)
        v = v.view(B, T, self.n_head, C // self.n_head).transpose(1, 2)
        if self.flash:
            y = F.scaled_dot_product_attention(q, k, v, attn_mask=None, dropout_p=self.dropout if self.training else 0, is_causal=True)
            #@shape y: [x.shape[0], self.n_head, x.shape[1], self.n_embd // self.n_head]
        else:
            att = (q @ k.transpose(-2, -1)) * (1.0 / math.sqrt(k.size(-1)))  #@shape@L201 att: [3, 4, 10, 10]
            causal = self.bias[:, :, :T, :T]  #@shape@L201 causal: [1, 1, 10, 10]
            att = att.masked_fill(causal == 0, float("-inf"))  #@shape@L201 att: [3, 4, 10, 10]
            att = F.softmax(att, dim=-1)
            att = self.attn_dropout(att)
            y = att @ v  #@shape@L201 y: [3, 4, 10, 8]
        y = y.transpose(1, 2).contiguous().view(B, T, C)  #@shape y: [x.shape[0], x.shape[1], self.n_embd]
        y = self.resid_dropout(self.c_proj(y))  #@shape y: [x.shape[0], x.shape[1], self.n_embd]
        #@requires none
        return y


class MLP(nn.Module):
    def __init__(self, config):
        super().__init__()
        self.c_fc = nn.Linear(config.n_embd, 4 * config.n_embd, bias=config.bias)
        self.gelu = nn.GELU()
        self.c_proj = nn.Linear(4 * config.n_embd, config.n_embd, bias=config.bias)
        self.dropout = nn.Dropout(config.dropout)

    def forward(self, x):
        h = self.c_fc(x)  #@shape h: [*x.shape[:-1], 4*config.n_embd]
        #@requires x.shape[-1] == config.n_embd
        a = self.gelu(h)  #@shape a: [*x.shape[:-1], 4*config.n_embd]
        out = self.dropout(self.c_proj(a))  #@shape out: [*x.shape[:-1], config.n_embd]
        #@requires none
        return out


class Block(nn.Module):
    def __init__(self, config):
        super().__init__()
        self.ln_1 = LayerNorm(config.n_embd, bias=config.bias)
        self.attn = CausalSelfAttention(config)
        self.ln_2 = LayerNorm(config.n_embd, bias=config.bias)
        self.mlp = MLP(config)

    def forward(self, x):
        a = self.attn(self.ln_1(x))  #@shape a: [x.shape[0], x.shape[1], self.attn.n_embd]
        h = x + a  #@shape h: [x.shape[0], x.shape[1], self.attn.n_embd]
        m = self.mlp(self.ln_2(h))  #@shape m: [x.shape[0], x.shape[1], self.attn.n_embd]
        out = h + m  #@shape out: [x.shape[0], x.shape[1], self.attn.n_embd]
        return out


class GPT(nn.Module):
    def __init__(self, config):
        super().__init__()
        assert config.vocab_size is not None
        assert config.block_size is not None
        self.config = config
        self.transformer = nn.ModuleDict(dict(
            wte=nn.Embedding(config.vocab_size, config.n_embd),
            wpe=nn.Embedding(config.block_size, config.n_embd),
            drop=nn.Dropout(config.dropout),
            h=nn.ModuleList([Block(config) for _ in range(config.n_layer)]),
            ln_f=LayerNorm(config.n_embd, bias=config.bias),
        ))
        self.lm_head = nn.Linear(config.n_embd, config.vocab_size, bias=False)
        self.transformer.wte.weight = self.lm_head.weight  # weight tying
        self.apply(self._init_weights)

    def _init_weights(self, module):
        if isinstance(module, nn.Linear):
            torch.nn.init.normal_(module.weight, mean=0.0, std=0.02)
            if module.bias is not None:
                torch.nn.init.zeros_(module.bias)
        elif isinstance(module, nn.Embedding):
            torch.nn.init.normal_(module.weight, mean=0.0, std=0.02)

    def forward(self, idx, targets=None):
        device = idx.device
        b, t = idx.size()  #@dim b: idx.shape[0]; t: idx.shape[1]
        #@requires idx.dim() == 2
        assert t <= self.config.block_size, f"Cannot forward sequence of length {t}, block size is only {self.config.block_size}"
        pos = torch.arange(0, t, dtype=torch.long, device=device)  #@shape pos: [idx.shape[1]]
        tok_emb = self.transformer.wte(idx)  #@shape tok_emb: [idx.shape[0], idx.shape[1], self.config.n_embd]
        pos_emb = self.transformer.wpe(pos)  #@shape pos_emb: [idx.shape[1], self.config.n_embd]
        x = self.transformer.drop(tok_emb + pos_emb)  #@shape x: [idx.shape[0], idx.shape[1], self.config.n_embd]
        for block in self.transformer.h:
            x = block(x)
        x = self.transformer.ln_f(x)  #@shape x: [idx.shape[0], idx.shape[1], self.config.n_embd]
        if targets is not None:
            logits = self.lm_head(x)  #@shape logits: [idx.shape[0], idx.shape[1], self.config.vocab_size]
            #@requires none
            flat = logits.view(-1, logits.size(-1))  #@shape flat: [idx.shape[0]*idx.shape[1], self.config.vocab_size]
            loss = F.cross_entropy(flat, targets.view(-1), ignore_index=-1)  #@shape loss: []
        else:
            # inference: only the last position
            logits = self.lm_head(x[:, [-1], :])  #@shape logits: [idx.shape[0], 1, self.config.vocab_size]
            loss = None
        return logits, loss  #@shape logits: [idx.shape[0], ?, self.config.vocab_size]

    @torch.no_grad()
    def generate(self, idx, max_new_tokens, temperature=1.0, top_k=None):
        for _ in range(max_new_tokens):
            idx_cond = idx if idx.size(1) <= self.config.block_size else idx[:, -self.config.block_size:]
            logits, _ = self(idx_cond)
            logits = logits[:, -1, :] / temperature  #@shape@L202 logits: [1, 17]
            if top_k is not None:
                v, _ = torch.topk(logits, min(top_k, logits.size(-1)))
                logits[logits < v[:, [-1]]] = -float("Inf")
            probs = F.softmax(logits, dim=-1)  #@shape@L202 probs: [1, 17]
            idx_next = torch.multinomial(probs, num_samples=1)  #@shape@L202 idx_next: [1, 1]
            idx = torch.cat((idx, idx_next), dim=1)
        return idx


def wrong_width():
    config = GPTConfig(block_size=16, vocab_size=17, n_layer=1, n_head=4, n_embd=32)
    attn = CausalSelfAttention(config)
    x = torch.randn(2, 5, 48)
    qkv = attn.c_attn(x)  #@shape-error
    return qkv


if __name__ == "__main__":
    config = GPTConfig(block_size=16, vocab_size=17, n_layer=2, n_head=4, n_embd=32, dropout=0.0, bias=True)
    model = GPT(config)
    model.eval()
    idx = torch.randint(0, 17, (3, 10))
    targets = torch.randint(0, 17, (3, 10))
    logits, loss = model(idx, targets)  #@shape logits: [3, 10, 17]; loss: []
    last, no_loss = model(idx)  #@shape last: [3, 1, 17]; no_loss: none
    for block in model.transformer.h:
        block.attn.flash = False
    slow_logits, slow_loss = model(idx, targets)  #@shape slow_logits: [3, 10, 17]; slow_loss: []
    out = model.generate(idx[:1, :4], 3)  #@shape out: [1, 7]
    try:
        wrong_width()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

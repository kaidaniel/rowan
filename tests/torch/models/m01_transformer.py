# Transformer encoder-decoder written from scratch (Vaswani et al. 2017, "Attention Is All
# You Need"; layout after the Annotated Transformer, post-norm residuals as in the paper).
# Checked: the scaled dot-product attention helper on parameters of unknown rank (conditional
# matmul shape), head split / merge with view + transpose, Linear requirements on q/k/v,
# the feed-forward block, the sinusoidal table and its slice by sequence length (`min`),
# padding and causal masks with broadcasting, encoder / decoder layers and stacks (rank fixed
# by unpacking), the generator to the vocabulary; concrete shapes for a small model run on
# (2, 5) source and (2, 7) target tokens, and one q @ k^T with mismatched head dims.
import math

import torch
import torch.nn as nn
import torch.nn.functional as F


def attention(query, key, value, mask=None, dropout_p=0.0, training=False):
    d_k = query.size(-1)  #@dim d_k: query.shape[-1]
    scores = query @ key.transpose(-2, -1) / math.sqrt(d_k)
    #@shape scores: [*broadcast(query.shape[:-2], key.shape[:-2]), query.shape[-2], key.shape[-2]] if query.dim() >= 2
    #@requires key.dim() >= 2; query.shape[-1] == key.shape[-1] if query.dim() >= 2
    #@shape@L263 scores: [2, 4, 5, 5]
    #@shape@L264 scores: [2, 4, 7, _]
    if mask is not None:
        scores = scores.masked_fill(mask == 0, -1e9)
    p_attn = scores.softmax(dim=-1)  #@shape@L263 p_attn: [2, 4, 5, 5]
    p_attn = F.dropout(p_attn, p=dropout_p, training=training)
    out = p_attn @ value  #@shape@L263 out: [2, 4, 5, 8]; p_attn: [2, 4, 5, 5]
    return out, p_attn


def subsequent_mask(size):
    upper = torch.triu(torch.ones(1, size, size), diagonal=1)  #@shape upper: [1, size, size]
    keep = upper == 0  #@shape keep: [1, size, size]
    return keep


def make_masks(src, tgt):
    src_mask = (src != 0).unsqueeze(-2)  #@shape src_mask: [*src.shape[:-1], 1, src.shape[-1]]
    B, T = tgt.shape  #@dim B: tgt.shape[0]; T: tgt.shape[1]
    #@requires tgt.dim() == 2
    pad_mask = (tgt != 0).unsqueeze(-2)  #@shape pad_mask: [tgt.shape[0], 1, tgt.shape[1]]
    causal = subsequent_mask(T)  #@shape causal: [1, tgt.shape[1], tgt.shape[1]]
    tgt_mask = pad_mask & causal  #@shape tgt_mask: [tgt.shape[0], tgt.shape[1], tgt.shape[1]]
    return src_mask, tgt_mask


def sinusoidal_table(max_len, d_model):
    pe = torch.zeros(max_len, d_model)  #@shape pe: [max_len, d_model]
    position = torch.arange(0, max_len).unsqueeze(1)  #@shape position: [max_len, 1]
    div_term = torch.exp(torch.arange(0, d_model, 2) * (-math.log(10000.0) / d_model))
    pe[:, 0::2] = torch.sin(position * div_term)
    pe[:, 1::2] = torch.cos(position * div_term)
    return pe


class MultiHeadAttention(nn.Module):
    def __init__(self, d_model, n_heads, dropout):
        super().__init__()
        assert d_model % n_heads == 0
        self.d_model = d_model
        self.n_heads = n_heads
        self.d_k = d_model // n_heads
        self.w_q = nn.Linear(d_model, d_model)
        self.w_k = nn.Linear(d_model, d_model)
        self.w_v = nn.Linear(d_model, d_model)
        self.w_o = nn.Linear(d_model, d_model)
        self.dropout_p = dropout

    def split_heads(self, x):
        B, L, _ = x.shape  #@dim B: x.shape[0]; L: x.shape[1]
        heads = x.view(B, L, self.n_heads, self.d_k)  #@shape heads: [x.shape[0], x.shape[1], self.n_heads, self.d_k]
        return heads.transpose(1, 2)

    def forward(self, query, key, value, mask=None):
        B, Tq, _ = query.shape  #@dim B: query.shape[0]; Tq: query.shape[1]
        #@requires query.dim() == 3
        q_lin = self.w_q(query)  #@shape q_lin: [query.shape[0], query.shape[1], self.d_model]
        #@requires query.shape[2] == self.d_model
        k_lin = self.w_k(key)  #@shape k_lin: [*key.shape[:-1], self.d_model]
        #@requires key.shape[-1] == self.d_model
        v_lin = self.w_v(value)  #@shape v_lin: [*value.shape[:-1], self.d_model]
        #@requires value.shape[-1] == self.d_model
        q = self.split_heads(q_lin)  #@shape q: [query.shape[0], self.n_heads, query.shape[1], self.d_k]
        k = self.split_heads(k_lin)  #@shape k: [key.shape[0], self.n_heads, key.shape[1], self.d_k]
        v = self.split_heads(v_lin)  #@shape v: [value.shape[0], self.n_heads, value.shape[1], self.d_k]
        if mask is not None:
            mask = mask.unsqueeze(1)  # the same mask for every head
        x, _ = attention(q, k, v, mask, self.dropout_p, self.training)  #@shape@L263 x: [2, 4, 5, 8]
        merged = x.transpose(1, 2).contiguous().view(B, Tq, self.d_model)
        #@shape merged: [query.shape[0], query.shape[1], self.d_model]
        out = self.w_o(merged)  #@shape out: [query.shape[0], query.shape[1], self.d_model]
        #@requires none
        return out


class PositionwiseFeedForward(nn.Module):
    def __init__(self, d_model, d_ff, dropout):
        super().__init__()
        self.d_model = d_model
        self.d_ff = d_ff
        self.w_1 = nn.Linear(d_model, d_ff)
        self.w_2 = nn.Linear(d_ff, d_model)
        self.dropout = nn.Dropout(dropout)

    def forward(self, x):
        h = F.relu(self.w_1(x))  #@shape h: [*x.shape[:-1], self.d_ff]
        #@requires x.shape[-1] == self.d_model
        out = self.w_2(self.dropout(h))  #@shape out: [*x.shape[:-1], self.d_model]
        #@requires none
        return out


class PositionalEncoding(nn.Module):
    def __init__(self, d_model, dropout, max_len):
        super().__init__()
        self.d_model = d_model
        self.max_len = max_len
        self.dropout = nn.Dropout(p=dropout)
        self.register_buffer("pe", sinusoidal_table(max_len, d_model).unsqueeze(0))

    def forward(self, x):
        T = x.size(1)  #@dim T: x.shape[1]
        pos = self.pe[:, :T]  #@shape pos: [1, min(x.shape[1], self.max_len), self.d_model]
        #@shape@L263 pos: [1, 5, 32]
        y = self.dropout(x + pos)  #@shape@L263 y: [2, 5, 32]
        return y


class Embeddings(nn.Module):
    def __init__(self, vocab, d_model):
        super().__init__()
        self.lut = nn.Embedding(vocab, d_model)
        self.d_model = d_model

    def forward(self, tokens):
        e = self.lut(tokens) * math.sqrt(self.d_model)  #@shape e: [*tokens.shape, self.d_model]
        #@requires none
        return e


class EncoderLayer(nn.Module):
    def __init__(self, d_model, n_heads, d_ff, dropout):
        super().__init__()
        self.d_model = d_model
        self.self_attn = MultiHeadAttention(d_model, n_heads, dropout)
        self.feed_forward = PositionwiseFeedForward(d_model, d_ff, dropout)
        self.norm1 = nn.LayerNorm(d_model)
        self.norm2 = nn.LayerNorm(d_model)
        self.dropout = nn.Dropout(dropout)

    def forward(self, x, mask):
        a = self.self_attn(x, x, x, mask)  #@shape a: [x.shape[0], x.shape[1], self.d_model]
        h = self.norm1(x + self.dropout(a))  #@shape h: [x.shape[0], x.shape[1], self.d_model]
        f = self.feed_forward(h)  #@shape f: [x.shape[0], x.shape[1], self.d_model]
        out = self.norm2(h + self.dropout(f))  #@shape out: [x.shape[0], x.shape[1], self.d_model]
        #@shape@L263 out: [2, 5, 32]
        return out


class DecoderLayer(nn.Module):
    def __init__(self, d_model, n_heads, d_ff, dropout):
        super().__init__()
        self.d_model = d_model
        self.self_attn = MultiHeadAttention(d_model, n_heads, dropout)
        self.src_attn = MultiHeadAttention(d_model, n_heads, dropout)
        self.feed_forward = PositionwiseFeedForward(d_model, d_ff, dropout)
        self.norm1 = nn.LayerNorm(d_model)
        self.norm2 = nn.LayerNorm(d_model)
        self.norm3 = nn.LayerNorm(d_model)
        self.dropout = nn.Dropout(dropout)

    def forward(self, x, memory, src_mask, tgt_mask):
        a = self.self_attn(x, x, x, tgt_mask)  #@shape a: [x.shape[0], x.shape[1], self.d_model]
        h1 = self.norm1(x + self.dropout(a))  #@shape h1: [x.shape[0], x.shape[1], self.d_model]
        c = self.src_attn(h1, memory, memory, src_mask)  #@shape c: [x.shape[0], x.shape[1], self.d_model]
        #@shape@L264 c: [2, 7, 32]
        h2 = self.norm2(h1 + self.dropout(c))  #@shape h2: [x.shape[0], x.shape[1], self.d_model]
        f = self.feed_forward(h2)
        out = self.norm3(h2 + self.dropout(f))  #@shape out: [x.shape[0], x.shape[1], self.d_model]
        return out


class Encoder(nn.Module):
    def __init__(self, n_layers, d_model, n_heads, d_ff, dropout):
        super().__init__()
        self.d_model = d_model
        self.layers = nn.ModuleList([EncoderLayer(d_model, n_heads, d_ff, dropout) for _ in range(n_layers)])
        self.norm = nn.LayerNorm(d_model)

    def forward(self, x, mask):
        B, S, _ = x.shape  #@dim B: x.shape[0]; S: x.shape[1]
        h = x
        for layer in self.layers:
            h = layer(h, mask)  #@shape h: [x.shape[0], x.shape[1], self.d_model]
        out = self.norm(h)  #@shape out: [x.shape[0], x.shape[1], self.d_model]
        return out


class Decoder(nn.Module):
    def __init__(self, n_layers, d_model, n_heads, d_ff, dropout):
        super().__init__()
        self.d_model = d_model
        self.layers = nn.ModuleList([DecoderLayer(d_model, n_heads, d_ff, dropout) for _ in range(n_layers)])
        self.norm = nn.LayerNorm(d_model)

    def forward(self, x, memory, src_mask, tgt_mask):
        B, T, _ = x.shape  #@dim B: x.shape[0]; T: x.shape[1]
        h = x
        for layer in self.layers:
            h = layer(h, memory, src_mask, tgt_mask)  #@shape h: [x.shape[0], x.shape[1], self.d_model]
        out = self.norm(h)  #@shape out: [x.shape[0], x.shape[1], self.d_model]
        return out


class Transformer(nn.Module):
    def __init__(self, src_vocab, tgt_vocab, d_model, n_heads, d_ff, n_layers, dropout, max_len):
        super().__init__()
        self.d_model = d_model
        self.tgt_vocab = tgt_vocab
        self.src_embed = Embeddings(src_vocab, d_model)
        self.tgt_embed = Embeddings(tgt_vocab, d_model)
        self.src_pos = PositionalEncoding(d_model, dropout, max_len)
        self.tgt_pos = PositionalEncoding(d_model, dropout, max_len)
        self.encoder = Encoder(n_layers, d_model, n_heads, d_ff, dropout)
        self.decoder = Decoder(n_layers, d_model, n_heads, d_ff, dropout)
        self.generator = nn.Linear(d_model, tgt_vocab)

    def encode(self, src, src_mask):
        emb = self.src_embed(src)  #@shape emb: [*src.shape, self.d_model]
        x = self.src_pos(emb)  #@shape@L263 x: [2, 5, 32]
        return self.encoder(x, src_mask)

    def decode(self, memory, src_mask, tgt, tgt_mask):
        emb = self.tgt_embed(tgt)  #@shape emb: [*tgt.shape, self.d_model]
        y = self.tgt_pos(emb)  #@shape@L264 y: [2, 7, 32]
        return self.decoder(y, memory, src_mask, tgt_mask)

    def generate(self, h):
        logits = self.generator(h)  #@shape logits: [*h.shape[:-1], self.tgt_vocab]
        #@requires h.shape[-1] == self.d_model
        log_probs = F.log_softmax(logits, dim=-1)  #@shape log_probs: [*h.shape[:-1], self.tgt_vocab]
        return log_probs

    def forward(self, src, tgt, src_mask, tgt_mask):
        memory = self.encode(src, src_mask)
        out = self.decode(memory, src_mask, tgt, tgt_mask)
        return self.generate(out)


def mismatched_heads():
    q = torch.randn(2, 4, 5, 8)
    k = torch.randn(2, 4, 6, 16)
    scores = q @ k.transpose(-2, -1)  #@shape-error
    return scores


if __name__ == "__main__":
    model = Transformer(13, 11, d_model=32, n_heads=4, d_ff=64, n_layers=2, dropout=0.1, max_len=50)
    model.eval()
    src = torch.randint(1, 13, (2, 5))
    tgt = torch.randint(1, 11, (2, 7))
    src_mask, tgt_mask = make_masks(src, tgt)  #@shape src_mask: [2, 1, 5]; tgt_mask: [2, 7, 7]
    memory = model.encode(src, src_mask)  #@shape memory: [2, 5, 32]
    dec = model.decode(memory, src_mask, tgt, tgt_mask)  #@shape dec: [2, 7, 32]
    log_probs = model(src, tgt, src_mask, tgt_mask)  #@shape log_probs: [2, 7, 11]
    try:
        mismatched_heads()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

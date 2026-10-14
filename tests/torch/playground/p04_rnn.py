# The owner's pytorch-playground `rnn-model.py`: a character-level RNN language model (a hand-
# written RNN cell unrolled over T) trained on `xiyouji.txt`, an input rowan cannot see (its
# text, vocabulary and V are unknown; the oracle runs on a small stand-in text next to this
# file). Changed for the oracle: `device` is "cpu" (was "mps") and STEPS 40 (was 500; OneCycleLR needs 40 for its 5% warm-up).
# Checked: the owner's shape comments, in each function's own terms (its parameters'
# dims, `d_h` / `d` / `V` of `__init__`) and in the training call's concrete sizes. Two of the
# owner's notes are about `nn.RNN` (commented out), not MyRNN: its `h` is (B, d), not (1, B, d).
"""Character-level RNN language model. Same data, tokeniser, batching and training loop as bigram-model.py;
only the model class and generate() differ."""
from math import floor

import torch
import torch.nn.functional as F
from torch import Tensor, nn
from pathlib import Path

torch.manual_seed(0)
device = "cpu"

text: str = Path("xiyouji.txt").read_text()

vocab: list[str] = sorted(set(text))
idx: dict[str, int] = {c: i for i, c in enumerate(vocab)}
V: int = len(vocab)


def encode(s: str) -> Tensor:
    out = torch.tensor([idx[c] for c in s], dtype=torch.long, device=device) # (T,)
    #@shape out: [?]
    #@todo shape out: [len(s)] (the length of a list comprehension over an unknown str)
    return out


def decode(ids: Tensor) -> str:
    return "".join(vocab[i] for i in ids.tolist())


def get_batch(data: Tensor, B: int, T: int) -> tuple[Tensor, Tensor]:
    i = torch.randint(0, len(data) - T - 1, (B,))          # (B, ): Random starting positions
    #@shape i: [B]
    x = torch.stack([data[j : j + T] for j in i])          # (B, T)
    #@shape x: [B, _, *data.shape[1:]]
    #@todo shape x: [B, T, *data.shape[1:]] (a slice j:j+T of an unknown start: T long only when j + T <= len(data), which randint's bound gives)
    y = torch.stack([data[j + 1 : j + T + 1] for j in i])  # (B, T)
    #@shape y: [B, _, *data.shape[1:]]
    return x, y


class MyRNN(nn.Module):
    def __init__(self, d_in: int, d_h: int) -> None:
        super().__init__()
        self.W_x = nn.Linear(d_in, d_h)
        self.W_h = nn.Linear(d_h, d_h, bias=False)

    def forward(self, x: Tensor, h: Tensor | None = None) -> tuple[Tensor, Tensor]:
        # x: (B, T, d_in)
        B, T, _ = x.shape
        #@dim B: x.shape[0]; T: x.shape[1]
        #@requires x.dim() == 3
        if h is None:
            h = x.new_zeros(B, self.W_h.in_features) # h: (B, d_h)
            #@shape h: [x.shape[0], d_h]
        xw = self.W_x(x) # xw: (B, T, d_h)
        #@shape xw: [x.shape[0], x.shape[1], d_h]
        #@shape@L143 xw: [64, _, 256]
        outs: list[Tensor] = [] # list[(B, d_h)]
        for t in range(T):
            hw = self.W_h(h) # h: (B, d_h)
            #@shape hw: [*_, d_h]
            #@shape@L143 hw: [64, 256]
            xw_t = xw[:, t, :] # xw_t: (B, d_h)
            #@shape xw_t: [x.shape[0], d_h]
            h = torch.tanh(xw_t + hw) # h: (B, d_h)
            #@shape h: [*_, d_h]
            #@shape@L143 h: [64, 256]
            outs.append(h)
        # len(outs) == T
        return torch.stack(outs, dim=1), h # (B, T, d_h), (B, d_h)



class RNNLM(nn.Module):
    def __init__(self, V: int, d: int) -> None:
        super().__init__()
        self.emb = nn.Embedding(V, d)                       # token id -> vector
        # self.rnn = nn.RNN(d, d, batch_first=True)           # h_t = tanh(W_x x_t + W_h h_{t-1} + b)
        self.rnn = MyRNN(d, d)
        self.head = nn.Linear(d, V)                         # hidden state -> logits

    def forward(self, ids: Tensor, h: Tensor | None = None) -> tuple[Tensor, Tensor]:
        # ids: (B, T); h: (1, B, d) or None (= zeros)
        emb = self.emb(ids)                                 # emb turns each int into a vector - (B, T): int -> (B, T, d): float
        #@shape emb: [*ids.shape, d]
        out, h = self.rnn(emb, h)                           # out: (B, T, d) — every h_t; h: (1, B, d) — last one
        #@todo shape out: [*_, d]; h: [*_, d] (on a run that returns, MyRNN's loop ran: with no step its stack of no tensors raises; rowan keeps the caller's h of any shape)
        #@shape@L143 out: [64, _, 256]; h: [64, 256]
        return self.head(out), h                            # logits: (B, T, V)

    @torch.no_grad()
    def generate(self, ids: Tensor, n: int, temperature: float = 1.0, greedy: bool = False) -> Tensor:
        # Run the prompt once to build the state, then feed one token at a time from that state.
        # ids: (B, T)
        logits, h = self(ids)                               # (B, T, V), (1, B, d)
        #@shape logits: [ids.shape[0], _, V]; h: [ids.shape[0], d]
        #@todo shape logits: [ids.shape[0], ids.shape[1], V] (the length of the list MyRNN.forward stacks: T appends in a loop of unknown length)
        logits = logits[:, -1, :]                              # (B, V): only the last position matters
        #@shape logits: [ids.shape[0], V]
        for _ in range(n):
            if greedy:
                next_id = logits.argmax(dim=-1, keepdim=True) # (B, 1) - the index of the V-vector that has the max value.
                #@shape next_id: [ids.shape[0], 1]
            else:
                probs = F.softmax(logits / temperature, dim=-1)     # still (B, V) but now every V-vector sums to 1.
                #@shape probs: [ids.shape[0], V]
                next_id = torch.multinomial(probs, num_samples=1)   # (B, 1)
                #@shape next_id: [ids.shape[0], 1]
            ids = torch.cat([ids, next_id], dim=1)
            #@shape ids: [ids.shape[0], _]
            logits, h = self(next_id, h)                    # one step: (B, 1, V); state carried in h
            #@shape logits: [ids.shape[0], 1, V]; h: [ids.shape[0], d]
            logits = logits[:, -1]
            #@shape logits: [ids.shape[0], V]
        return ids # (B, T+n)


split = floor(0.9 * len(text))
train, test = encode(text[:split]), encode(text[split:])
#@shape train: [?]; test: [?]

model = RNNLM(V, d=256).to(device)
STEPS = 40
optim = torch.optim.AdamW(model.parameters(), lr=1e-3)
sched = torch.optim.lr_scheduler.OneCycleLR(optim, max_lr=3e-3, total_steps=STEPS, pct_start=0.05)

print(f"vocab: {len(vocab)}")
print(f"parameters: {sum(p.numel() for p in model.parameters())}")

for step in range(STEPS):
    x, y = get_batch(train, B=64, T=32) # x: (B, T): int; y: (B, T): int
    #@shape x: [64, _]; y: [64, _]
    #@todo shape x: [64, 32]; y: [64, 32] (a slice j:j + T of an unknown start in get_batch)
    logits: Tensor
    logits, _ = model(x) # logits: (B, T, V)
    #@shape logits: [64, _, _]
    #@todo shape logits: [64, 32, V] (V = len(vocab) of the unknown text: a number rowan has no name for)
    loss = F.cross_entropy(logits.view(-1, V), y.view(-1))
    #@shape loss: []
    optim.zero_grad()
    loss.backward()
    optim.step()
    sched.step()
    if step % 100 == 0:
        model.eval()
        with torch.no_grad():
            vals = []
            for _ in range(10):                             # average several val batches: less noisy
                xv, yv = get_batch(test, B=64, T=32)
                #@shape xv: [64, _]; yv: [64, _]
                lv, _ = model(xv)
                #@shape lv: [64, _, _]
                vals.append(F.cross_entropy(lv.view(-1, V), yv.view(-1)))
            val = torch.stack(vals).mean()
            #@shape val: []
        model.train()
        print(f"{step:4d}  train {loss.item():.3f}  val {val.item():.3f}  ppl {val.exp().item():.1f}")


def complete(prompt: str, n: int, **kw) -> str:
    ids = encode(prompt)[None, :]                           # (1, T)
    #@shape@L177 ids: [1, 3]
    completion = model.generate(ids, n, **kw)               # (1, T+n)
    #@shape@L177 completion: [1, ?]
    #@todo shape@L177 completion: [1, 63] (60 steps of a cat along dim 1 in a loop that is not unrolled)
    return decode(completion[0])


print(complete("孫悟空", n=60))
print(complete("孫悟空", n=60, greedy=True))

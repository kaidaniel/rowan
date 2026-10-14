# The owner's pytorch-playground `pytorch_playground/gpt.py` (here the package case's entry; it
# imports Block from pytorch_playground/transformers.py): a GPT over characters, trained on
# `xiyouji.txt`, an input rowan cannot see (its text, vocabulary and V are unknown; the oracle
# runs on a small stand-in text next to this file). Changed for the oracle: `device` is "cpu"
# (was "mps"), the MPS memory print runs only where there is MPS, d=64 (was 256) and STEPS 40
# (was 1800). Checked: the owner's shape comments, in each method's own terms and in the
# driver's concrete sizes. One owner note asked for `T[64, 64, ?]` at the validation batch:
# the batch `xv` is [64, 64]; the model's output on it, `lv`, is [64, 64, V].
from torch import nn, Tensor
import torch

from pytorch_playground.transformers import Block

class GPT(nn.Module):
    def __init__(self, d: int, V: int, context_length: int, n_heads: int, n_layers: int) -> None:
        super().__init__()
        self.context_length = context_length
        self.tok_emb = nn.Embedding(V, d)
        self.pos_emb = nn.Embedding(context_length, d)
        self.blocks = nn.ModuleList([Block(d, n_heads, context_length) for _ in range(n_layers)])
        self.head = nn.Linear(d, V)
        self.head.weight = self.tok_emb.weight
        self.ln_f = nn.LayerNorm(d)
        self.apply(self._init_weights)

    @staticmethod
    def _init_weights(m: nn.Module) -> None:
        if isinstance(m, nn.Linear):
            nn.init.normal_(m.weight, mean=0.0, std=0.02)
            if m.bias is not None:
                nn.init.zeros_(m.bias)
        elif isinstance(m, nn.Embedding):
            nn.init.normal_(m.weight, mean=0.0, std=0.02)

    def forward(self, ids: Tensor) -> Tensor:
        # ids: (B, T) int
        # out: (B, T, V)
        _ , T = ids.shape
        #@dim T: ids.shape[1]
        #@requires ids.dim() == 2
        x = self.tok_emb(ids) # (B, T, d)
        #@shape x: [ids.shape[0], ids.shape[1], d]
        p = self.pos_emb(torch.arange(T, device=ids.device)) # (T, d)
        #@shape p: [ids.shape[1], d]
        x = x + p # (B, T, d) - p is broadcast over B.
        #@shape x: [ids.shape[0], ids.shape[1], d]
        for block in self.blocks:
            x = block(x) # (B, T, d) - but rowan doesn't even see this as a Tensor!
            #@shape x: [*_, d]
            #@todo shape x: [ids.shape[0], ids.shape[1], d] (Head.forward is past its budget by the entry runs: 12 blocks of 8 heads per model call; and the heads' rank fact is lost after MultiHead's loop)
        return self.head(self.ln_f(x)) # (B, T, V)
    
    @torch.no_grad()
    def generate(self, ids: Tensor, n: int, temperature: float=1.0, greedy: bool=False) -> Tensor:
        # ids: (B, T) int
        for _ in range(n):
            ctx = ids[:, -self.context_length:]             # (B, ≤T_max): the model's entire memory
            #@todo shape ctx: [ids.shape[0], _, *ids.shape[2:]] (ids joined over the loop's rounds loses its rank: before the loop it is open, after a round 2-D)
            #@todo shape ctx: [ids.shape[0], min(ids.shape[1], self.context_length), *ids.shape[2:]] (a slice by an unknown negative start)
            curr = self(ctx)[:, -1, :]                       # (B, V)
            #@todo shape curr: [ids.shape[0], V] (Head.forward is past its budget by the entry runs: 12 blocks of 8 heads per model call; and the heads' rank fact is lost after MultiHead's loop)
            if greedy:
                next_id = curr.argmax(dim=-1, keepdim=True)
                #@shape next_id: [*_, 1]
                #@todo shape next_id: [ids.shape[0], 1] (curr above)
            else:
                next_id = torch.multinomial(nn.functional.softmax(curr / temperature, -1), 1)
                #@todo shape next_id: [ids.shape[0], 1] (curr above)
            ids = torch.cat([ids, next_id], dim=1)
            #@todo shape ids: [ids.shape[0], _] (ctx above)
        return ids                                                # (B, T+n)
    

if __name__ == "__main__":
    from pathlib import Path
    from math import floor

    torch.manual_seed(0)
    device = "cpu"

    text: str = Path("xiyouji.txt").read_text()
    vocab: list[str] = sorted(set(text))
    idx: dict[str, int] = {c: i for i, c in enumerate(vocab)} # here, rowan says "{**?}", but later (line 65) it has inferred three keys from 'idx' (孫,悟,空)? Maybe that's just the way the hover is formulated and what rowan actually inferred is relative to that function - I don't know.
    #@row idx: definite={} open
    V: int = len(vocab)

    def encode(s: str) -> Tensor:
        out = torch.tensor([idx[c] for c in s], dtype=torch.long, device=device) # (T,)
        #@shape out: [len(s)]
        # (a comprehension over the str s: one item per character)
        return out # rowan gets this as T[*] when I feel like it should be able to get T[?] at least (definitely 1-dimensional)?


    def decode(ids: Tensor) -> str:
        return "".join(vocab[i] for i in ids.tolist())


    def get_batch(data: Tensor, B: int, T: int) -> tuple[Tensor, Tensor]:
        i = torch.randint(0, len(data) - T - 1, (B,))          # (B, ): Random starting positions
        #@shape i: [B]
        x = torch.stack([data[j : j + T] for j in i])          # (B, T) - rowan gets this as unknown
        #@shape x: [B, T, *data.shape[1:]]
        # (j lies in 0..len(data) - T - 2, randint's range: each window is T long)
        y = torch.stack([data[j + 1 : j + T + 1] for j in i])  # (B, T) - rowan gets this as unknown
        #@shape y: [B, _, *data.shape[1:]]
        return x, y
    
    split = floor(0.9 * len(text))
    train, test = encode(text[:split]), encode(text[split:])
    #@shape train: [?]; test: [?]

    model = GPT(d=64, V=V, context_length=68, n_heads=8, n_layers=12).to(device)
    STEPS = 40
    optim = torch.optim.AdamW(model.parameters(), lr=3e-4, weight_decay=0.1)
    sched = torch.optim.lr_scheduler.OneCycleLR(optim, max_lr=1e-3, total_steps=STEPS, pct_start=0.05)

    print(f"vocab: {V:,}")
    print(f"parameters: {sum(p.numel() for p in model.parameters()):,}")

    for step in range(STEPS):
        x, y = get_batch(train, B=68, T=64) # x: (B, T): int; y: (B, T): int
        #@shape x: [68, _]; y: [68, _]
        #@todo shape x: [68, 64]; y: [68, 64] (a slice j:j + T of an unknown start in get_batch: train's length is not known, so j + T <= len(train) is not shown)
        logits: Tensor = model(x) # (B, T, V)
        #@todo shape logits: [68, 64, V] (Head.forward is past its budget by the entry runs: 12 blocks of 8 heads per model call; and the heads' rank fact is lost after MultiHead's loop)
        loss = nn.functional.cross_entropy(logits.view(-1, V), y.view(-1))
        #@shape loss: []
        optim.zero_grad()
        loss.backward()
        optim.step()
        sched.step()
        if step == 1 and torch.backends.mps.is_available():
            print(f"{torch.mps.current_allocated_memory()=:,}")
        if step % 100 == 0:
            model.eval()
            with torch.no_grad():
                vals = []
                for _ in range(10):                             # average several val batches: less noisy
                    xv, yv = get_batch(test, B=64, T=64) # I feel like rowan should be able to get the right number of dimensions here, so it should be T[64, 64, ?]. If it can show T[64, 64, len(idx)] it would be even more awesome.
                    #@shape xv: [64, _]; yv: [64, _]
                    lv = model(xv)
                    #@todo shape lv: [64, 64, V] (Head.forward is past its budget by the entry runs: 12 blocks of 8 heads per model call; and the heads' rank fact is lost after MultiHead's loop)
                    vals.append(nn.functional.cross_entropy(lv.view(-1, V), yv.view(-1)))
                val = torch.stack(vals).mean()
                #@shape val: []
            model.train()
            print(f"{step:4d}  train {loss.item():.3f}  val {val.item():.3f}  ppl {val.exp().item():.1f}")


def complete(prompt: str, n: int, **kw) -> str:
    ids = encode(prompt)[None, :]                           # (1, T)
    #@shape ids: [1, _]
    #@todo shape ids: [1, 3] (encode's comprehension over the prompt: its run here is past encode's budget)
    completion = model.generate(ids, n, **kw)               # (1, T+n)
    #@shape completion: [1, ?]
    #@todo shape completion: [1, 63] (60 steps of a cat along dim 1 in a loop that is not unrolled)
    return decode(completion[0])

print("sampling:")
print(complete("孫悟空", n=60, temperature=0.3))
print("greedy:")
print(complete("孫悟空", n=60, greedy=True))

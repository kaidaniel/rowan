# The owner's pytorch-playground `bigram-model.py`, unchanged but for this header and the
# directives: a character bigram model (an Embedding V x V of next-token logits) trained on a
# literal Lorem-ipsum text, then sampled. Checked: the encoded splits, the batches (rows of a
# slice by a random start: [B, T]), the logits and loss shapes in training, and generate()'s
# shapes in the one call `complete` makes ([1, 19] prompt, 10 new tokens).
import torch, torch.nn
from torch import Tensor, tensor
from math import floor

class Bigram(torch.nn.Module):
    def __init__(self, V: int) -> None:
        super().__init__()
        self.next_token_logits = torch.nn.Embedding(V, V)

    def forward(self, current: Tensor) -> Tensor:
        return self.next_token_logits(current)

    @torch.no_grad()
    def generate(self, ids: Tensor, n: int) -> Tensor:
        for _ in range(n):
            logits = self(ids[:, -1])
            #@shape@L91 logits: [1, 32]
            probs = torch.softmax(logits, dim=1)
            #@shape@L91 probs: [1, 32]
            next_token = torch.multinomial(probs, num_samples=1)
            #@shape@L91 next_token: [1, 1]
            ids = torch.cat([ids, next_token], dim=1)
            #@shape@L91 ids: [1, ?]
        return ids


text: str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum. Sed ut perspiciatis unde omnis iste natus error sit voluptatem accusantium doloremque laudantium, totam rem aperiam, eaque ipsa quae ab illo inventore veritatis et quasi architecto beatae vitae dicta sunt explicabo. Nemo enim ipsam voluptatem quia voluptas sit aspernatur aut odit aut fugit, sed quia consequuntur magni dolores eos qui ratione voluptatem sequi nesciunt. Neque porro quisquam est, qui dolorem ipsum quia dolor sit amet, consectetur, adipisci velit, sed quia non numquam eius modi tempora incidunt ut labore et dolore magnam aliquam quaerat voluptatem. Ut enim ad minima veniam, quis nostrum exercitationem ullam corporis suscipit laboriosam, nisi ut aliquid ex ea commodi consequatur? Quis autem vel eum iure reprehenderit qui in ea voluptate velit esse quam nihil molestiae consequatur, vel illum qui dolorem eum fugiat quo voluptas nulla pariatur?"
vocab: list[str] = sorted(set(text))
idx: dict[str, int] = {w: i for i, w in enumerate(vocab)}
#@row idx: present={" ", ",", ".", "?", "D", "E", "L", "N", "Q", "S", "U", "a", "b", "c", "d", "e", "f", "g", "h", "i", "l", "m", "n", "o", "p", "q", "r", "s", "t", "u", "v", "x"} definite={" ", ",", ".", "?", "D", "E", "L", "N", "Q", "S", "U", "a", "b", "c", "d", "e", "f", "g", "h", "i", "l", "m", "n", "o", "p", "q", "r", "s", "t", "u", "v", "x"} closed

def encode(s: str) -> Tensor:
    return tensor([idx[c] for c in s], dtype=torch.long)

def decode(ids: Tensor) -> str:
    return "".join(vocab[i] for i in ids.tolist())

def get_batch(data: Tensor, B: int, T: int) -> tuple[Tensor, Tensor]:
    i = torch.randint(0, len(data) - T - 1, (B,))
    #@shape i: [B]
    x = torch.stack([data[j : j + T] for j in i])
    #@shape x: [B, _, *data.shape[1:]]
    #@todo shape x: [B, T, *data.shape[1:]] (a slice j:j+T of an unknown start: T long only when j + T <= len(data), which randint's bound gives)
    #@shape@L63 x: [32, 32]
    y = torch.stack([data[j + 1 : j + 1 + T] for j in i])
    #@shape y: [B, _, *data.shape[1:]]
    return x, y

split = floor(0.9 * len(text))
#@dim split: 1179
train = encode(text[:split])
#@shape train: [1179]
test = encode(text[split:])
#@shape test: [132]
model = Bigram(len(vocab))
optim = torch.optim.AdamW(model.parameters(), lr=4e-3)
for step in range(3_000):
    x, y = get_batch(train, 32, 32)
    #@shape x: [32, 32]; y: [32, 32]
    model.train()
    x_hat = model(x)
    #@shape x_hat: [32, 32, 32]
    loss = torch.nn.functional.cross_entropy(
        x_hat.view(-1, len(vocab)), 
        y.view(-1)
    )
    optim.zero_grad()
    loss.backward()
    optim.step()
    model.eval()
    if step % 1_000 == 0:
        with torch.no_grad():
            xv, yv = get_batch(test, 32, 32)
            #@shape xv: [32, 32]; yv: [32, 32]
            val = torch.nn.functional.cross_entropy(
                model(xv).view(-1, len(vocab)),
                yv.view(-1)
            )
            print(f"{step:4d}  train {loss.item():.3f}  val {val.item():.3f}  ppl {val.exp().item():.1f}")

def complete(prompt: str, n: int) -> str:
    ids = encode(prompt)[None, :]
    #@shape@L91 ids: [1, 19]
    return decode(model.generate(ids, n)[0])

print(complete("Lorem ipsum dolor s", n=10))

# The owner's pytorch-playground `key-memory-net.py`, unchanged but for this header and the
# directives: a key-value memory network over a literal knowledge base (12 facts, 12
# questions, 25 words, 11 answer entities). Checked: the vocabulary and candidate indexes,
# the owner's shape comments (N = M = 12 memories / questions, V = 25, C = 11, d = 16), both
# in forward()'s own terms (open arguments) and in the training and answer() calls.
"""Key-Value Memory Network (Miller et al. 2016), minimal version.

Task: answer a question from a small "knowledge base" of facts.
  memory slot i:  key  = bag of words of the fact sentence   (used for MATCHING the question)
                  value = the answer entity in that fact       (used for READING OUT)
  q             :  bag of words of the question
  alpha_i = softmax_i( q . k_i )        -- soft, differentiable lookup
  o       = sum_i alpha_i v_i           -- read-out
  logits  = o . E[c]  for every candidate answer c
Trained end-to-end with cross-entropy on the answer.
"""
import torch, torch.nn as nn, torch.nn.functional as F
torch.manual_seed(0)

# ---------------------------------------------------------------- tiny knowledge base
facts = [                          # (sentence, answer entity)
    ("blade runner directed by scott",     "scott"),
    ("blade runner released 1982",         "1982"),
    ("blade runner starring ford",         "ford"),
    ("alien directed by scott",            "scott"),
    ("alien released 1979",                "1979"),
    ("alien starring weaver",              "weaver"),
    ("heat directed by mann",              "mann"),
    ("heat released 1995",                 "1995"),
    ("heat starring pacino",               "pacino"),
    ("jaws directed by spielberg",         "spielberg"),
    ("jaws released 1975",                 "1975"),
    ("jaws starring scheider",             "scheider"),
]
questions = [                      # (question, answer)
    ("who directed blade runner",  "scott"),
    ("when was blade runner released", "1982"),
    ("who starred in blade runner", "ford"),
    ("who directed alien",         "scott"),
    ("when was alien released",    "1979"),
    ("who starred in alien",       "weaver"),
    ("who directed heat",          "mann"),
    ("when was heat released",     "1995"),
    ("who starred in heat",        "pacino"),
    ("who directed jaws",          "spielberg"),
    ("when was jaws released",     "1975"),
    ("who starred in jaws",        "scheider"),
]

# ---------------------------------------------------------------- vocab (words) and candidates (answers)
words = sorted({w for s, _ in facts + questions for w in s.split()})
w2i = {w: i for i, w in enumerate(words)}
#@row w2i: present={"1975", "1979", "1982", "1995", "alien", "blade", "by", "directed", "ford", "heat", "in", "jaws", "mann", "pacino", "released", "runner", "scheider", "scott", "spielberg", "starred", "starring", "was", "weaver", "when", "who"} definite={"1975", "1979", "1982", "1995", "alien", "blade", "by", "directed", "ford", "heat", "in", "jaws", "mann", "pacino", "released", "runner", "scheider", "scott", "spielberg", "starred", "starring", "was", "weaver", "when", "who"} closed
cands = sorted({a for _, a in facts})
c2i = {c: i for i, c in enumerate(cands)}
#@row c2i: present={"1975", "1979", "1982", "1995", "ford", "mann", "pacino", "scheider", "scott", "spielberg", "weaver"} definite={"1975", "1979", "1982", "1995", "ford", "mann", "pacino", "scheider", "scott", "spielberg", "weaver"} closed

def bow(sentence: str) -> torch.Tensor:                 # bag of words -> (V,) multi-hot
    v = torch.zeros(len(words)); v[[w2i[w] for w in sentence.split() if w in w2i]] = 1; return v   # unknown words ignored
    #@shape v: [25]

K_bow = torch.stack([bow(s) for s, _ in facts])          # (N, V)  N memory slots
#@shape K_bow: [12, 25]
V_idx = torch.tensor([c2i[a] for _, a in facts])         # (N,)    value = answer entity id
#@shape V_idx: [12]
Q_bow = torch.stack([bow(q) for q, _ in questions])      # (M, V)
#@shape Q_bow: [12, 25]
Y     = torch.tensor([c2i[a] for _, a in questions])     # (M,)
#@shape Y: [12]

# ---------------------------------------------------------------- model
class KVMemNet(nn.Module):
    def __init__(self, V: int, C: int, d: int) -> None:
        super().__init__()
        self.A = nn.Linear(V, d, bias=False)   # embeds bag-of-words -> d  (questions and KEYS)
        self.E = nn.Embedding(C, d)            # embeds answer entities -> d (VALUES and candidates)

    def forward(self, q_bow, k_bow, v_idx):
        q = self.A(q_bow)                       # (M, d)   question
        #@shape q: [*q_bow.shape[:-1], d]
        #@shape@L108 q: [12, 16]
        #@shape@L123 q: [1, 16]
        k = self.A(k_bow)                       # (N, d)   keys
        #@shape k: [*k_bow.shape[:-1], d]
        #@shape@L108 k: [12, 16]
        v = self.E(v_idx)                       # (N, d)   values
        #@shape v: [*v_idx.shape, d]
        #@shape@L108 v: [12, 16]
        scores = q @ k.T                        # (M, N)   every question vs every memory
        #@shape@L108 scores: [12, 12]
        #@shape@L123 scores: [1, 12]
        alpha = F.softmax(scores, dim=-1)       # (M, N)   rows sum to 1
        #@shape@L108 alpha: [12, 12]
        #@shape@L123 alpha: [1, 12]
        o = alpha @ v                           # (M, d)   read-out: weighted sum of values
        #@shape@L108 o: [12, 16]
        #@shape@L123 o: [1, 16]
        logits = o @ self.E.weight.T            # (M, C)   score every candidate answer
        #@shape logits: [*_, C]
        #@shape@L108 logits: [12, 11]
        #@shape@L123 logits: [1, 11]
        return logits, alpha

model = KVMemNet(len(words), len(cands), d=16)
opt = torch.optim.Adam(model.parameters(), lr=0.05)

for step in range(300):
    logits, alpha = model(Q_bow, K_bow, V_idx)
    #@shape logits: [12, 11]; alpha: [12, 12]
    loss = F.cross_entropy(logits, Y)
    #@shape loss: []
    opt.zero_grad(); loss.backward(); opt.step()
    if step % 50 == 0:
        acc = (logits.argmax(-1) == Y).float().mean()
        #@shape acc: []
        print(f"step {step:3d}  loss {loss.item():.3f}  acc {acc:.2f}")

# ---------------------------------------------------------------- predict on a new question (unseen wording)
def answer(question: str) -> None:
    with torch.no_grad():
        emb = bow(question)[None] # (1, V)
        #@shape emb: [1, 25]
        logits, alpha = model(emb, K_bow, V_idx)      # (1, C), (1, N)
        #@shape logits: [1, 11]; alpha: [1, 12]
    probs = F.softmax(logits[0], dim=-1)
    #@shape probs: [11]
    top3 = alpha[0].topk(3)
    print(f"\nQ: {question}")
    print(f"A: {cands[probs.argmax()]}  (p={probs.max():.2f})")
    for w, i in zip(top3.values, top3.indices):
        print(f"   attends {w:.2f} -> '{facts[i][0]}'")

answer("who directed jaws")             # seen wording
answer("jaws director")                 # unseen wording; "director" is out-of-vocab and ignored
answer("blade runner release year")     # unseen wording
answer("who directed heat and alien")   # ambiguous: two movies
answer("which movie did mann direct")

# ---------------------------------------------------------------- inspect: which memory does each question attend to?
with torch.no_grad():
    logits, alpha = model(Q_bow, K_bow, V_idx)
    #@shape logits: [12, 11]; alpha: [12, 12]
for (q, a), al, pred in zip(questions, alpha, logits.argmax(-1)):
    #@shape al: [12]; pred: []
    top = al.argmax().item()
    print(f"{q:32s} -> {cands[pred]:10s} (true {a:10s})  attends {al[top]:.2f} to '{facts[top][0]}'")
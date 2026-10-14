# The owner's pytorch-playground `linear-relu-linear-model.py`, unchanged but for this header
# and the directives: a hand-set Linear-ReLU-Linear that answers "what is the capital of …".
# Checked: the literal vocabulary and its index, the one-hot input [K, V] flattened to
# [K*V], the logits [V] (the owner's comments), and the answer: one of the vocabulary's words.
import torch, torch.nn as nn

vocab = ["what-is-the", "capital", "of-France", "of-Germany", "Berlin", "Paris", "<bad>", "<pad>"]
V, K = len(vocab), 3                     # vocab size 8, context window 3
#@dim V: 8; K: 3
idx = {w: i for i, w in enumerate(vocab)}
#@row idx: present={"what-is-the", "capital", "of-France", "of-Germany", "Berlin", "Paris", "<bad>", "<pad>"} definite={"what-is-the", "capital", "of-France", "of-Germany", "Berlin", "Paris", "<bad>", "<pad>"} closed

# Input: 3 one-hot vectors of size 8, flattened -> 24 numbers. Position p, token t sits at p*V + t.
model = nn.Sequential(nn.Linear(K * V, 2), nn.ReLU(), nn.Linear(2, V))
fc1, fc2 = model[0], model[2]

with torch.no_grad():
    # --- hidden layer: two "pattern detectors" ---
    fc1.weight.zero_(); fc1.bias.fill_(-2.0)          # needs all 3 slots to match to exceed 0
    fc1.weight[0, 0*V + idx["what-is-the"]] = 1       # h0 fires for  what-is-the | capital | of-France
    #@ ok what-is-the
    fc1.weight[0, 1*V + idx["capital"]]     = 1
    fc1.weight[0, 2*V + idx["of-France"]]   = 1
    #@ ok of-France
    fc1.weight[1, 0*V + idx["what-is-the"]] = 1       # h1 fires for  what-is-the | capital | of-Germany
    fc1.weight[1, 1*V + idx["capital"]]     = 1
    fc1.weight[1, 2*V + idx["of-Germany"]]  = 1
    #@ ok of-Germany
    # --- output layer: map detectors to next-token logits ---
    fc2.weight.zero_(); fc2.bias.zero_()
    fc2.bias[idx["<bad>"]] = 5                        # default answer
    #@ ok <bad>
    fc2.weight[idx["Paris"],  0] = 10                 # h0 -> Paris
    #@ ok Paris
    fc2.weight[idx["Berlin"], 1] = 10                 # h1 -> Berlin
    #@ ok Berlin
    fc2.weight[idx["<bad>"],  0] = -10                # any detector firing suppresses <bad>
    fc2.weight[idx["<bad>"],  1] = -10

def encode(tokens):                                   # list[str] (len 3) -> (24,)
    x = torch.zeros(K, V)
    #@shape x: [3, 8]
    for p, t in enumerate(tokens):
        x[p, idx[t]] = 1
    return x.flatten()

def next_token(tokens):
    with torch.no_grad():
        logits = model(encode(tokens))                # (8,)
        #@shape logits: [8]
        return vocab[logits.argmax().item()], logits

tests = [
    ["what-is-the", "capital", "of-France"],
    ["what-is-the", "capital", "of-Germany"],
    ["what-is-the", "capital", "capital"],
    ["capital", "what-is-the", "of-France"],          # right words, wrong order
    ["<pad>", "<pad>", "of-France"],
]
for t in tests:
    tok, logits = next_token(t)
    #@shape logits: [8]
    #@value tok: exactly={"what-is-the", "capital", "of-France", "of-Germany", "Berlin", "Paris", "<bad>", "<pad>"}
    print(f"{' '.join(t):45s} -> {tok:8s}  logits={logits.tolist()}")

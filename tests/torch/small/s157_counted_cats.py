# `ids = torch.cat([ids, nxt], dim=d)` in each round of `for _ in range(T)`, `nxt` of size c
# along d in every round: after the loop ids is c * T longer along d (T a tensor's dim or a
# literal). Not when the rounds may add different sizes, the loop may stop early, or T is a
# named int (it may be negative: no round runs).
import torch


def grows(x):
    B, T = x.shape
    ids = torch.zeros(2, 1)
    for _ in range(T):
        nxt = torch.ones(2, 1)
        ids = torch.cat([ids, nxt], dim=1)
    return ids  #@shape ids: [2, x.shape[1] + 1]


def literal_rounds():
    ids = torch.zeros(2, 1)
    for _ in range(30):
        nxt = torch.ones(2, 1)
        ids = torch.cat((ids, nxt), dim=-1)
    return ids  #@shape ids: [2, 31]


def two_per_round(x):
    B, T = x.shape
    ids = torch.zeros(3, 2)
    for _ in range(T):
        ids = torch.cat([ids, torch.ones(3, 2)], 1)
    return ids  #@shape ids: [3, _]
    #@todo shape ids: [3, 2*x.shape[1] + 2] (a part that is not a name: its size in every round is not looked up)


def rebound_after(x):
    # the first round cats the 1-wide nxt from before the loop, later ones a 2-wide one
    B, T = x.shape
    ids = torch.zeros(2, 1)
    nxt = torch.ones(2, 1)
    for _ in range(T):
        ids = torch.cat([ids, nxt], dim=1)
        nxt = torch.ones(2, 2)
    return ids  #@shape ids: [2, ?]


def sizes_differ(x, wide):
    B, T = x.shape
    ids = torch.zeros(2, 1)
    for _ in range(T):
        if wide:
            nxt = torch.ones(2, 2)
        else:
            nxt = torch.ones(2, 1)
        ids = torch.cat([ids, nxt], dim=1)
    return ids  #@shape ids: [2, ?]


def stops(x):
    B, T = x.shape
    ids = torch.zeros(2, 1)
    for t in range(T):
        nxt = torch.ones(2, 1)
        ids = torch.cat([ids, nxt], dim=1)
        if t == 3:
            break
    return ids  #@shape ids: [2, ?]


def named_count(n: int):
    ids = torch.zeros(2, 1)
    for _ in range(n):
        nxt = torch.ones(2, 1)
        ids = torch.cat([ids, nxt], dim=1)
    return ids  #@shape ids: [2, ?]


if __name__ == "__main__":
    x = torch.randn(2, 20)
    print(grows(x).shape, literal_rounds().shape, two_per_round(x).shape, rebound_after(x).shape)
    print(sizes_differ(x, True).shape, sizes_differ(x, False).shape, stops(x).shape, named_count(-3).shape, named_count(4).shape)

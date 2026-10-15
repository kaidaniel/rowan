# A list appended to once in each round of `for _ in range(T)` is T longer after the loop (T a
# dim): `len`, `torch.stack`. Not when a round may skip the append (`continue`, an append under
# a test), may end the loop early (`break`), or when the list may change otherwise (another
# name for it, a call given it). An append after the loop adds one.
import torch


def steps(x):
    B, T, C = x.shape
    outs = []
    for t in range(T):
        h = x[:, t] * 2
        outs.append(h)
    n = len(outs)  #@dim n: x.shape[1]
    s = torch.stack(outs, dim=1)  #@shape s: [x.shape[0], x.shape[1], x.shape[2]]
    return s


def from_two(x):
    B, T, C = x.shape
    outs = [x[:, 0], x[:, 0]]
    for t in range(T):
        outs.append(x[:, t])
    s = torch.stack(outs)  #@shape s: [x.shape[1] + 2, x.shape[0], x.shape[2]]
    return s


def skips(x):
    B, T, C = x.shape
    outs = []
    for t in range(T):
        if t % 2:
            continue
        outs.append(x[:, t])
    s = torch.stack(outs, dim=1)  #@shape s: [x.shape[0], ?, x.shape[2]]
    return s


def under_test(x):
    B, T, C = x.shape
    outs = []
    for t in range(T):
        if t > 0:
            outs.append(x[:, t])
    s = torch.stack(outs, dim=1)  #@shape s: [x.shape[0], ?, x.shape[2]]
    return s


def stops(x):
    B, T, C = x.shape
    outs = []
    for t in range(T):
        outs.append(x[:, t])
        if t == 2:
            break
    s = torch.stack(outs, dim=1)  #@shape s: [x.shape[0], ?, x.shape[2]]
    return s


def aliased(x):
    B, T, C = x.shape
    outs = []
    more = outs
    for t in range(T):
        outs.append(x[:, t])
        more.append(x[:, t])
    s = torch.stack(outs, dim=1)  #@shape s: [x.shape[0], ?, x.shape[2]]
    return s


def add_one(xs, v):
    xs.append(v)


def given_away(x):
    B, T, C = x.shape
    outs = []
    for t in range(T):
        outs.append(x[:, t])
        add_one(outs, x[:, t])
    s = torch.stack(outs, dim=1)  #@shape s: [x.shape[0], ?, x.shape[2]]
    return s


def appended_after(x):
    B, T, C = x.shape
    outs = []
    for t in range(T):
        outs.append(x[:, t])
    outs.append(x[:, 0])
    s = torch.stack(outs, dim=1)  #@shape s: [x.shape[0], x.shape[1] + 1, x.shape[2]]
    return s


if __name__ == "__main__":
    x = torch.randn(2, 20, 3)
    for fn in (steps, from_two, skips, under_test, stops, aliased, given_away, appended_after):
        print(fn.__name__, tuple(fn(x).shape))

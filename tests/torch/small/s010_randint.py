# torch.randint(low, high, size) / randint(high, size) / randint_like: the shape is the size argument
import torch


def concrete():
    a = torch.randint(0, 10, (3, 4))  #@shape a: [3, 4]
    b = torch.randint(10, (5,))  #@shape b: [5]
    c = torch.randint(low=2, high=9, size=(2, 2, 2))  #@shape c: [2, 2, 2]
    d = torch.randint(5, size=(7,))  #@shape d: [7]
    return a, b, c, d


def symbolic(vocab, B, T):
    ids = torch.randint(0, vocab, (B, T))  #@shape ids: [B, T]
    labels = torch.randint(vocab, size=(B,))  #@shape labels: [B]
    coin = torch.randint(0, 2, (T, 1))  #@shape coin: [T, 1]
    other = torch.randint_like(ids, 0, vocab)  #@shape other: [B, T]
    return ids, labels, coin, other


if __name__ == "__main__":
    concrete()
    symbolic(100, 2, 5)

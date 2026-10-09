# F.cross_entropy (mean -> [], reduction="none" -> [N] or [N, d1]), F.mse_loss, nn.CrossEntropyLoss, a target mismatch
import torch
import torch.nn.functional as F
from torch import nn


def ce(logits):
    N, C = logits.shape
    target = torch.randint(0, C, (N,))  #@shape target: [logits.shape[0]]
    loss = F.cross_entropy(logits, target)  #@shape loss: []
    #@requires none
    per = F.cross_entropy(logits, target, reduction="none")  #@shape per: [logits.shape[0]]
    return loss, per


def ce_seq(logits):
    N, C, L = logits.shape
    target = torch.randint(0, C, (N, L))
    per = F.cross_entropy(logits, target, reduction="none")  #@shape per: [logits.shape[0], logits.shape[2]]
    return per


def mse(pred):
    target = torch.zeros_like(pred)
    loss = F.mse_loss(pred, target)  #@shape loss: []
    per = F.mse_loss(pred, target, reduction="none")  #@shape per: [*pred.shape]
    total = F.mse_loss(pred, target, reduction="sum")  #@shape total: []
    return loss, per, total


def module_loss(N, C):
    loss = nn.CrossEntropyLoss()(torch.randn(N, C), torch.randint(0, C, (N,)))  #@shape loss: []
    return loss


def bad_target():
    loss = F.cross_entropy(torch.randn(4, 10), torch.randint(0, 10, (5,)))  #@shape-error
    return loss


if __name__ == "__main__":
    ce(torch.randn(4, 3))
    ce_seq(torch.randn(2, 5, 7))
    mse(torch.randn(3, 2))
    module_loss(6, 4)
    try:
        bad_target()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass

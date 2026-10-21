# Hypothesis: a tensor dim divided by what may be 0 may raise ZeroDivisionError, and a try sees it.
# `n // 0` of a dim surely raises (a dim is an int); `n // m` of two dims may (a dim may be 0:
# torch.zeros(0)); `n // 2` and `n % 3` cannot. Before, a dim's division raised nothing.
# Expectations were written from the program text before running the engine.
import torch


def by_zero(x):
    n = x.shape[0]
    try:
        r = n // 0
        how = "ok"
    except ZeroDivisionError:
        how = "zero"
    return how
    #@general how: exactly={"zero"}


def by_dim(x, y):
    n = x.shape[0]
    m = y.shape[0]
    try:
        r = n // m
        how = "ok"
    except ZeroDivisionError:
        how = "zero"
    return how
    #@general how: exactly={"ok", "zero"}


def by_literal(x):
    n = x.shape[0]
    try:
        r = n // 2 + n % 3
        how = "ok"
    except ZeroDivisionError:
        how = "zero"
    return how
    #@general how: exactly={"ok"}


if __name__ == "__main__":
    by_zero(torch.randn(3, 4))
    by_dim(torch.randn(3), torch.zeros(0))
    by_literal(torch.randn(5))

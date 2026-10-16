# Arithmetic on dims is Python's int arithmetic: `%` takes the divisor's sign (7 % -2 is -1,
# -7 % 2 is 1) and products do not wrap at 64 bits (2**40 * 2**40 is 2**80, not 0).
import torch


def mods(x):
    n = x.shape[0]
    assert n == 7
    a = n % -2
    #@dim a: -1
    b = (n - 14) % 2
    #@dim b: 1
    c = n // -2
    #@dim c: -4
    return a, b, c


def big(x):
    n = x.shape[0]
    m = n * 2**40
    p = m * 2**40
    #@dim p: ?
    # (2**80 * n is the exact value; rowan's dims hold 64-bit coefficients: not 0, unknown)
    t = torch.zeros(p - p + 2)
    #@shape t: [?]
    # (an unknown dim does not cancel against itself)
    return p, t


if __name__ == "__main__":
    mods(torch.zeros(7))
    big(torch.zeros(3))

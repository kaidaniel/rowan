# A name that is a tensor on one path and None on the other (`None if c else torch.zeros(3)`)
# is either: `x is None` holds on some runs. rowan's join kept the tensor and dropped the None,
# so the `is None` branch was dead. A tensor joined with a str is no tensor at all.
# Expectations written from the program text.
import os

import torch


def pick(c):
    x = None if c else torch.zeros(3)
    if x is None:
        r = "none"
    else:
        r = "tensor"
        #@shape x: [3]
    return r


def accumulate(n):
    acc = None
    for i in range(n):
        t = torch.ones(2)
        acc = t if acc is None else torch.cat([acc, t])
    if acc is None:
        return "empty"
    return "full"


def mixed(c):
    y = "off" if c else torch.zeros(2)
    if isinstance(y, str):
        return "str"
    return "tensor"


flag = os.environ.get("S159_FLAG") == "1"
r1 = pick(flag)                  #@value r1: exactly={"none", "tensor"}
r2 = accumulate(int(os.environ.get("S159_N", "0")))
#@value r2: exactly={"empty", "full"}
r3 = mixed(flag)                 #@value r3: exactly={"str", "tensor"}

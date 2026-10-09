# Flag combinations, StrEnum (equal to its string value), members as dict keys
from enum import Enum, Flag, StrEnum, auto


class Perm(Flag):
    R = 4
    W = 2
    X = 1


class Mode(StrEnum):
    FAST = auto()
    SAFE = "safe"


class Tier(Enum):
    FREE = "free"
    PRO = "pro"


COST = {Tier.FREE: 0, Tier.PRO: 10}


def budget(mode):
    limits = {Mode.FAST: 10, Mode.SAFE: 1}
    return limits[mode]                          #@ ok fast


def cost(tier_name):
    return COST.get(Tier(tier_name), -1)


if __name__ == "__main__":
    rw = Perm.R | Perm.W
    v = rw.value                                 #@value v: exactly={6}
    has_w = Perm.W in rw                         #@value has_w: exactly={True}
    has_x = Perm.X in rw                         #@value has_x: exactly={False}
    f = Mode.FAST.value                          #@value f: exactly={fast}
    eq = Mode.FAST == "fast"                     #@value eq: exactly={True}
    b1 = budget(Mode.SAFE)                       #@value b1: exactly={1}
    b2 = budget("fast")                          #@value b2: exactly={10}
    limits = {Mode.FAST: 10, Mode.SAFE: 1}       #@row limits: present={fast,safe} definite={fast,safe} closed
    c = cost("pro")                              #@value c: exactly={10}
    s = f"{Mode.SAFE}"                           #@value s: exactly={safe}

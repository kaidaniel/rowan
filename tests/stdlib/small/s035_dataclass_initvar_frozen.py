# InitVar is no attribute, __post_init__ adds attributes, asdict sees only fields; frozen instances
from dataclasses import FrozenInstanceError, InitVar, asdict, dataclass, fields


@dataclass
class Account:
    owner: str
    raw_limit: InitVar[str]

    def __post_init__(self, raw_limit):
        self.limit = int(raw_limit) if raw_limit else 0
        if self.limit > 100:
            self.vip = True


@dataclass(frozen=True)
class Key:
    ns: str
    name: str


def summary(acc):
    _ = 0                                        #@row acc: present={owner,limit,vip} definite={owner,limit} absent={raw_limit} closed
    d = asdict(acc)                              #@row d: present={owner} definite={owner} absent={limit,vip} closed
    acc.limit                                    #@ ok limit
    return d


if __name__ == "__main__":
    a = Account("x", "500")
    b = Account("y", "")
    summary(a)
    summary(b)
    try:
        b.vip                                    #@ missing vip
    except AttributeError:
        pass
    try:
        a.raw_limit                              #@ missing raw_limit
    except AttributeError:
        pass
    fn = tuple(f.name for f in fields(Account))  #@value fn: exactly={("owner",)}
    k = Key("a", "b")
    blocked = False
    try:
        k.name = "c"
    except FrozenInstanceError:
        blocked = True
    #@value blocked: exactly={True}
    table = {Key("a", "b"): 1}
    v = table[k]                                 #@value v: exactly={1}

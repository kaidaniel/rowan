"""@dataclass(order=True) instances compare their fields as tuples (fields with
field(compare=False) left out), so max / min / sorted pick a known instance; IntEnum / StrEnum
members compare by value (with each other and with plain ints / strs)."""
import enum
from dataclasses import dataclass, field


class Level(enum.IntEnum):
    LOW = 1
    MID = 2
    HIGH = 3


class Tag(enum.StrEnum):
    A = "a"
    B = "b"


@dataclass(order=True)
class Task:
    prio: int
    due: str
    name: str = field(compare=False, default="")


def pick(tasks):
    return max(tasks).name, min(tasks).name, [t.name for t in sorted(tasks)]


if __name__ == "__main__":
    ts = [Task(2, "b", "x"), Task(1, "z", "y"), Task(2, "a", "w"), Task(2, "b", "v")]
    top, low, order = pick(ts)
    _ = 0                                        #@value top: exactly={"x"} ; low: exactly={"y"}
    first = order[0]                             #@value first: exactly={"y"}
    second = order[1]                            #@value second: exactly={"w"}
    lt = Task(1, "a") < Task(1, "b", "zzz")      #@value lt: exactly={True}
    le = Task(1, "a", "p") <= Task(1, "a", "q")  #@value le: exactly={True}
    e1 = Level.LOW < Level.HIGH                  #@value e1: exactly={True}
    e2 = Level.MID >= 3                          #@value e2: exactly={False}
    e3 = max([Level.MID, Level.HIGH, Level.LOW]).name   #@value e3: exactly={"HIGH"}
    e4 = Tag.B > Tag.A                           #@value e4: exactly={True}
    e5 = sorted([Level.HIGH, Level.LOW])[0] == 1   #@value e5: exactly={True}

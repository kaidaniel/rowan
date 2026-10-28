"""An annotation that would raise if evaluated (a module bound to None here, as an optional
import leaves it) is never evaluated by Python 3.14 when the function runs: a function run for
any call goes on with that parameter as an unknown. (Before, the entry run stopped at the
annotation: nothing in `add`, `size` or `scale` was claimed for any call.)

Expectations were written from the program text before running the engine."""
column = None


class Table:
    def __init__(self):
        self.cols = []

    def add(self, c: "column.Column"):
        self.cols.append(c)
        n = len(self.cols)
        #@general n: types={int}
        return n


def size(c: column.Column, k: int):
    m = k + 1
    #@general m: types={int}
    tag = "size"
    #@general tag: exactly={"size"}
    return c


def scale(c: "column.Column") -> "column.Column":
    out = 2
    #@general out: exactly={2}
    return out


t = Table()
r = t.add(3)
#@value r: exactly={1}
size(None, 2)
scale(5)

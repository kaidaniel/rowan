"""A receiver that is one of several objects whose classes run user code for the operation
(`__getitem__`, a property, a setter) runs one of them, never both in sequence. They ran one
after another on one state: the second saw the first's effects as definite (`log` held both
`a` and `b`), and the first's raise did not end its own path only."""
import sys


class A:
    def __init__(self, log):
        self.log = log

    def __getitem__(self, k):
        self.log["a"] = k
        raise KeyError(k)

    @property
    def size(self):
        self.log["pa"] = 1
        return 1


class B:
    def __init__(self, log):
        self.log = log

    def __getitem__(self, k):
        self.log["b"] = k
        return 1

    @property
    def size(self):
        self.log["pb"] = 2
        return 2


log = {}
obj = A(log) if len(sys.argv) > 1 else B(log)
n = obj.size  #@value n: exactly={1,2}
print(log)  #@row log: present={pa,pb} definite={} closed
v = obj["x"]  #@value v: exactly={1}
print(log)  #@row log: present={pa,pb,b} definite={b} closed

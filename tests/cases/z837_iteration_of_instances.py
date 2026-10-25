"""Unpacking an instance runs its `__iter__`; one with only `__len__` / `__getitem__` is iterated by
`__getitem__` from 0 until it raises IndexError."""
log = []


class Pair:
    def __iter__(self):
        log.append("iter")
        return iter([1, 2])


class Seq:
    def __len__(self):
        return 2

    def __getitem__(self, i):
        log.append("getitem")
        if i >= 2:
            raise IndexError(i)
        return i


x, y = Pair()
n1 = len(log)
#@value n1: exactly={1}
for v in Seq():
    pass
n2 = len(log)
#@value n2: includes={4}
#@todo value n2: exactly={4}  (`__getitem__` until IndexError: run as library code runs dunders, any number)

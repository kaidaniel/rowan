# A method nothing in the program calls is analysed for any caller, on an instance its
# `__init__` made — and any of its methods may have run since: an attribute another method
# sets may be there (not "never set"); one another method deletes may be gone.


class Saver:
    def __init__(self, value):
        self.value = value
        self.label = "x"

    def __enter__(self):
        self.saved = self.value
        return self

    def __exit__(self, *exc):
        return self.saved  #@ possibly saved

    def clear(self):
        del self.label

    def show(self):
        return self.label  #@ possibly label

    def peek(self):
        return self.value  #@ ok value


class Counter:
    def __init__(self):
        self._n = 0

    @property
    def n(self):
        return self._n

    @n.setter
    def n(self, v):
        self._n = v

    def bump(self):
        self.n += 1

    def show(self):
        return self.n  #@ ok n

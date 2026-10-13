__all__ = ["area", "Square"]


class Square:
    def __init__(self, side):
        self.side = side


def area(s):
    return s.side ** 2


helper = 3

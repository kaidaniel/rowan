"""A classmethod's run for any call takes `cls` as the class or one of its subclasses — the
classes, not instances of them: `cls(…)` makes an instance, `cls.__name__` is a class's name.
(It took instances made by each subclass's `__init__`: a thousand `__init__` runs for a
library's base class, and `cls(…)` an instance's call.)"""


class Shape:
    def __init__(self, size):
        self.size = size

    @classmethod
    def make(cls, size):
        obj = cls(size)
        made = type(obj).__name__
#@general made: exactly={"Shape", "Square"}
        name = cls.__name__
#@general name: exactly={"Shape", "Square"}
        return obj


class Square(Shape):
    def __init__(self, size):
        super().__init__(size)
        self.side = size


def build(size):
    s = Square.make(size)
#@row s: present={size, side} definite={size, side} closed
    return s.side


side = build(3)
plain = Shape.make(2)

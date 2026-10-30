"""A class statement may rebind the name of its own base (`class Model(Model):`), and a class
inherits its bases' metaclass: the base objects decide, not the names their statements wrote.
Here `User`'s metaclass is `Stamp`, through the second `Model` and the first; its `__init__`
rebinds `made` on each class it makes. Before, rowan looked the bases up again by the names in
their class statements, read `Model` as the second `Model` itself, and recursed until its stack
overflowed.

Expectations were written from the program text before running the engine."""


class Stamp(type):
    def __init__(cls, name, bases, attrs):
        super().__init__(name, bases, attrs)
        cls.made = name


class Model(metaclass=Stamp):
    made = "body"


class Model(Model):
    pass


class User(Model):
    made = "user body"


made = User.made
#@value made: includes={"User"}
again = Model.made
#@value again: includes={"Model"}

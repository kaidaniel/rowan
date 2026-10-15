# Two classes with the same name (click's `core.Option` and `parser.Option`): the instances the
# entry runs of their methods take are two objects, each with its own class's attributes (they
# were one object: the one made last had the attributes, a false "never set" for the other
# class's).


class Base:
    def describe(self):
        return self.tag


def make_a():
    class Option(Base):
        def __init__(self):
            self.tag = "a"
            self.name = "x"

        def get(self):
            return self.name  #@ ok name

    return None


make_a()


class Option(Base):
    def __init__(self):
        self.tag = "b"
        self.dest = 1

    def get_dest(self):
        return self.dest  #@ ok dest

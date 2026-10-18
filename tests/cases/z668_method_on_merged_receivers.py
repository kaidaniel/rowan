# A method called on objects of several factories, each made in a loop of unknown length
# (merged objects: each stands for several), whose helper closure calls a method of the
# receiver it captured, which calls a method of the receiver's part of one of several classes
# (click's Option.get_help_record → _write_opts → make_metavar → type.get_metavar). One run
# per receiver at each level nested the levels' receivers: a run for each merged receiver is
# no more exact (stores into it only join), so they are one call.
import sys


class IntType:
    def metavar(self):
        return "INT"


class TextType:
    def metavar(self):
        return "TEXT"


class FloatType:
    def metavar(self):
        return "FLOAT"


TYPES = [IntType(), TextType(), FloatType()]


class Option:
    def __init__(self, name, ty):
        self.name = name
        self.type = ty

    def make_metavar(self):
        return self.type.metavar()

    def help_record(self):
        def write_opts(prefix):
            return prefix + self.name + " " + self.make_metavar()

        return write_opts("-") + write_opts("--")


def make(tag):
    return [Option(f"{tag}{a}", TYPES[i % 3]) for i, a in enumerate(sys.argv)]


opts = make("a") + make("b") + make("c") + make("d") + make("e")
records = [o.help_record() for o in opts]
# (records: a list of str, first one of them)
first = records[0] if records else "-"
#@value first: types={str}
#@tabulations <= 60

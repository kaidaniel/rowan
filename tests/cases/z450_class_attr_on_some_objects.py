# An attribute found through the class on some of the objects a name may hold (a method of
# one class) and on none of the others: the read fails on some paths only, not on every one.
# `callable(None)` is False: a branch on it is decided.
import sys


class Command:
    def __init__(self, name):
        self.name = name


class Group(Command):
    def command(self):
        return 1

    size = 3


cli = Group("a") if len(sys.argv) > 1 else Command("b")
x = cli.command                                  #@ possibly command
n = cli.size                                     #@ possibly size


def pick(name=None):
    if callable(name):
        return "called"
    return "plain"


p = pick()
#@value p: exactly={"plain"}
q = pick(print)
#@value q: exactly={"called"}

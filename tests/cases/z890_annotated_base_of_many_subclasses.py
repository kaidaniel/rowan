"""A parameter annotated with a class of the program may be an instance of any of its
subclasses. For a class with many subclasses that override its methods, rowan stands in for
those instances with an unknown (their methods are calls it does not follow); `isinstance` of
such a parameter against one of the subclasses is then undecided. Before, the unknown was taken
for no instance of the program's classes, so `isinstance(it, Last)` was surely False and the
branch never ran for any call (a list of syntax nodes grouped by their kinds).

Expectations were written from the program text before running the engine."""


class Node:
    def kind(self):
        return "node"


class Space(Node):
    def kind(self):
        return "space"


class Comma(Node):
    def kind(self):
        return "comma"


class Note(Node):
    def kind(self):
        return "note"


class Last(Node):
    def kind(self):
        return "last"


def group(nodes: list[Node]):
    out = []
    for it in nodes:
        if isinstance(it, Last):
            tag = "last"
            #@general tag: exactly={"last"}
            out.append(tag)
        else:
            out.append("other")
    return out


def one(it: Node):
    if isinstance(it, (Space, Comma)):
        sep = "sep"
        #@general sep: exactly={"sep"}
        return sep
    if isinstance(it, Node):
        return "node"
    return "none"


tags = group([Space(), Last()])
first = one(Comma())
#@value first: exactly={"sep"}

"""A class deriving from `list` through another user class has `list`'s methods as well: the
external base is inherited (as in email's `TokenList(list)` and its subclasses), so its
instances' attribute and item rows are open, not closed with the user attributes only."""


class TokenList(list):
    token_type = None

    def __init__(self, *args, **kw):
        super().__init__(*args, **kw)
        self.defects = []


class Atom(TokenList):
    token_type = "atom"


class Word(Atom):
    pass


a = Atom()
a.append("x")
#@ sound append
w = Word()
w.extend(["y", "z"])
#@ sound extend
d = a.defects
#@ ok defects
t = w.token_type
#@value t: exactly={"atom"}
first = a[0]
#@ sound 0
last = w[-1]
#@ sound -1

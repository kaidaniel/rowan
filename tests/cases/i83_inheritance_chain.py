"""A class hierarchy with a mixin: constructors chained through explicit base calls and
`super()`, overridden methods extended with `super().describe()`, attributes added per level.

Expectations were written from the program text before running the engine."""


class Base:
    def __init__(self, name):
        self.name = name
        self.tags = []

    def describe(self):
        return {"name": self.name, "kind": self.kind()}

    def kind(self):
        return "base"


class Timestamped:
    def __init__(self):
        self.created = 0

    def touch(self):
        self.updated = 1


class Doc(Timestamped, Base):
    def __init__(self, name, body):
        Base.__init__(self, name)
        Timestamped.__init__(self)
        self.body = body

    def kind(self):
        return "doc"

    def describe(self):
        d = super().describe()
        d["length"] = len(self.body)
        return d


class Draft(Doc):
    def __init__(self, name):
        super().__init__(name, "")
        self.draft = True

    def describe(self):
        d = super().describe()
        d["draft"] = True
        if self.body:
            d["has_body"] = True
        return d


d = Doc("a", "text")                      #@row d: present={name,tags,created,body} definite={name,tags,created,body} closed
dd = d.describe()                         #@row dd: present={name,kind,length} definite={name,kind,length} closed
dr = Draft("b")                           #@row dr: present={name,tags,created,body,draft} definite={name,tags,created,body,draft} closed
# A draft's body is the empty string: the has_body branch is dead.
drd = dr.describe()                       #@row drd: present={name,kind,length,draft} definite={name,kind,length,draft} closed
dr.touch()
_ = 0                                     #@row dr: present={name,tags,created,body,draft,updated} definite={name,tags,created,body,draft,updated} closed
_ = 0                                     #@row d: present={name,tags,created,body} definite={name,tags,created,body} closed
b = Base("c").describe()                  #@row b: present={name,kind} definite={name,kind} closed
d.updated                                 #@ missing updated
drd["has_body"]                           #@ missing has_body
dd["draft"]                               #@ missing draft

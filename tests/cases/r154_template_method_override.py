"""Hypothesis: a base-class template method calling `self.hook()` dispatches on the
receiver's class (including `super()` chains in a grandchild); an object of unknown choice
between two classes keeps both outcomes.

Expectations were written from the program text before running the engine."""


class Base:
    def build(self):
        out = {"kind": self.kind()}
        self.extra(out)
        return out

    def kind(self):
        return "base"

    def extra(self, out):
        pass


class Child(Base):
    def kind(self):
        return "child"

    def extra(self, out):
        out["child_only"] = True


class Grand(Child):
    def extra(self, out):
        super().extra(out)
        out["grand"] = True


b = Base().build()                           #@row b: present={kind} definite={kind} closed
c = Child().build()                          #@row c: present={kind,child_only} definite={kind,child_only} closed
g = Grand().build()                          #@row g: present={kind,child_only,grand} definite={kind,child_only,grand} closed
objs = [Base(), Grand()]
x = objs[1].build()                          #@row x: present={kind,child_only,grand} definite={kind,child_only,grand} closed
y = (Base() if flag else Child()).build()    #@row y: present={kind,child_only} definite={kind} closed
c["grand"]                                   #@ missing grand

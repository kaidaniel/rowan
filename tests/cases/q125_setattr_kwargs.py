"""Hypothesis: `setattr` in a loop over literal kwargs sets exactly those attributes, and
a subclass forwarding `**kwargs` to `super().__init__` gets them too.

Expectations were written from the program text before running the engine."""


class Base:
    def __init__(self, **kwargs):
        self.kind = "base"
        for k, v in kwargs.items():
            setattr(self, k, v)


class Sub(Base):
    def __init__(self, name, **kwargs):
        super().__init__(**kwargs)
        self.name = name


b = Base(x=1, y=2)                        #@row b: present={kind,x,y} definite={kind,x,y} closed
s = Sub("s", z=3)                         #@row s: present={kind,z,name} definite={kind,z,name} closed
s.x                                       #@ missing x
b.name                                    #@ missing name

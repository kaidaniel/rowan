"""Hypothesis: a subclass that does not redefine a class-level dict shares the base's; one that
redefines it has its own.

Expectations were written from the program text before running the engine."""


class Base:
    config = {"base": 1}


class Same(Base):
    pass


class Own(Base):
    config = {"own": 1}


Same.config["s"] = 1
Own.config["o"] = 1
bc = Base.config                             #@row bc: present={base,s} definite={base,s} closed
oc = Own.config                              #@row oc: present={own,o} definite={own,o} closed

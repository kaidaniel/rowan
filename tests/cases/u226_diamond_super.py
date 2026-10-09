"""Hypothesis: cooperative `super()` in a diamond follows the MRO of the instance's class
(Both -> Left -> Right -> Base), so every class in it contributes once.

Expectations were written from the program text before running the engine."""


class Base:
    def describe(self):
        return {"base": True}


class Left(Base):
    def describe(self):
        d = super().describe()
        d["left"] = True
        return d


class Right(Base):
    def describe(self):
        d = super().describe()
        d["right"] = True
        return d


class Both(Left, Right):
    def describe(self):
        d = super().describe()
        d["both"] = True
        return d


r = Both().describe()                        #@row r: present={base,left,right,both} definite={base,left,right,both} closed
l = Left().describe()                        #@row l: present={base,left} definite={base,left} closed
l["right"]                                   #@ missing right

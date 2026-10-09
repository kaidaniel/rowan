"""Weakness: metaclasses. A metaclass that adds a class attribute: reading it is not
"definitely missing".

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""


class Meta(type):
    def __new__(mcs, name, bases, ns):
        ns["added"] = {"by": "meta"}
        return super().__new__(mcs, name, bases, ns)


class C(metaclass=Meta):
    pass


C.added                                      #@ sound added
C().added                                    #@ sound added

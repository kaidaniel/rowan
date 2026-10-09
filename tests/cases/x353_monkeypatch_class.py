"""Weakness: monkeypatching. A method added to a class after its definition is there on its
instances.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""


class C:
    pass


C.extra = lambda self: {"x": 1}
r = C().extra()                              #@row r: present={x} definite={x} closed
_ = 0                                        #@row r: possible={x}

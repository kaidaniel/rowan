"""Weakness: dynamic attributes. `setattr` with a name from data may add any attribute.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""


class O:
    def __init__(self):
        self.a = 1


o = O()
setattr(o, name_in, 2)
o.anything                                   #@ sound anything
getattr(o, other_in)
o.a                                          #@ ok a

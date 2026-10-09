"""Hypothesis: an instance attribute assigned in `__init__` shadows the class attribute of the
same name, per instance.

Expectations were written from the program text before running the engine."""


class C:
    opts = {"cls": True}

    def __init__(self, own):
        if own:
            self.opts = {"inst": True}


a = C(True)
b = C(False)
ao = a.opts                                  #@row ao: present={inst} definite={inst} closed
bo = b.opts                                  #@row bo: present={cls} definite={cls} closed

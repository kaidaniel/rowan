# `super().__init__()` reaching `object.__init__` (a class whose bases are all the program's)
# does nothing: what the instance holds is as it was (it was unknown code given the instance:
# every attribute, and all they reach, changed — scikit-base's BaseObject, 2.3M objects walked
# on sktime).


class Base:
    def __init__(self, cfg):
        self.cfg = cfg
        super().__init__()
        n = self.cfg["n"]  #@ ok cfg ; ok n
        #@value n: includes={1}
        # (includes: the instance the entry runs of Base's methods take is made by
        # `Base(cfg)` with any cfg)


class A(Base):
    def __init__(self):
        self.x = 1
        super().__init__({"n": 1})
        self.y = self.x  #@ ok x


a = A()
#@row a: present={x,cfg,y} definite={x,cfg,y} closed

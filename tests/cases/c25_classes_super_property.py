class Base:
    def __init__(self, cfg):
        self.cfg = cfg
        self.name = cfg["name"]                     #@ ok name

    @property
    def label(self):
        return self.cfg["label"]

    def describe(self):
        return self.extra


class Child(Base):
    def __init__(self, cfg):
        super().__init__(cfg)
        self.extra = cfg.get("extra", 0)


b = Base({"name": "b", "label": "L"})
c = Child({"name": "c"})
b.label                                             #@ ok label
c.label                                             #@ missing label
b.describe()                                        #@ missing extra
c.describe()                                        #@ ok extra
c.extra                                             #@ ok extra
b.extra                                             #@ missing extra

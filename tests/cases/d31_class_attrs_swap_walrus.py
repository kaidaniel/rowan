class Base:
    DEFAULTS = {"retries": 3, "timeout": 10}

    def __init__(self, **overrides):
        self.cfg = {**self.DEFAULTS, **overrides}

    def get(self, k):
        return self.cfg[k]


class Fast(Base):
    DEFAULTS = {"retries": 1}


b = Base(verbose=True)
b.cfg["timeout"]                     #@ ok timeout
b.cfg["verbose"]                     #@ ok verbose
f = Fast()
f.cfg["retries"]                     #@ ok retries
f.cfg["timeout"]                     #@ missing timeout
f.get("retries")                     #@ ok retries
f.get("timeout")                     #@ missing timeout

a, c = {"a": 1}, {"c": 1}
a, c = c, a
a["c"]                               #@ ok c
a["a"]                               #@ missing a

items = [{"n": 1, "tag": "x"}, {"n": 2}]
tagged = [it for it in items if (t := it.get("tag"))]
# The filter is decided per item (only the first has a tag), so the comprehension keeps
# positions: tagged[0] is the tagged item.
tagged[0]["n"]                       #@ ok n
tagged[0]["tag"]                     #@ ok tag

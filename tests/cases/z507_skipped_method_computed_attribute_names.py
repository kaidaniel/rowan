# A method past its budget that reaches attributes of its receiver by a name built from a
# template (scikit-base's flags: `setattr(self, f"{flag_attr_name}_dynamic", …)`): as unknown
# code it may change the attribute its arguments name (`_config_dynamic`), or with an argument
# rowan cannot read any `…_dynamic`, not the receiver's others nor what they hold (the receiver
# as a whole escaping made every attribute, and everything they reach, changed: sktime walked
# 38M objects for them).
import os

#@limits budget=4
#@havoc <= 10


class Flags:
    def __init__(self):
        self.big = {"k%d" % i: {"v": i} for i in range(20)}
        self.cfg = {"a": 1}

    def set_flags(self, flag_attr_name="_flags", **flag_dict):
        dynamic = f"{flag_attr_name}_dynamic"
        if hasattr(self, dynamic):
            getattr(self, dynamic).update(flag_dict)
        else:
            setattr(self, dynamic, dict(flag_dict))
        return self


f = Flags()
f.set_flags(x=1)
f.set_flags(y=2)
f.set_flags(flag_attr_name="_config", z=3)
c = f.cfg["a"]  #@ ok cfg ; ok a
#@value c: exactly={1}
v = f.big["k3"]["v"]  #@ ok big
#@value v: exactly={3}
d = f._flags_dynamic  #@ ok _flags_dynamic
#@row d: present={x,y} definite={x,y} closed
e = f._config_dynamic  #@ sound _config_dynamic
f.set_flags(flag_attr_name=os.environ.get("ROWAN_CASE_FLAGS", "_env"), w=4)
g = f._flags_dynamic  #@ sound _flags_dynamic
#@row g: present={x,y} open
h = f.cfg["a"]  #@ ok cfg ; ok a


class Cache:
    def __init__(self):
        self._store = {"a": 1}

    def reset(self, name="store"):
        n = 1
        m = 2
        setattr(self, f"_{name}", {})


c0 = Cache()
c0.reset()
c0.reset()
c1 = Cache()
old = c1._store
c1.reset()
# (past the budget: the attribute is bound again; the dict it held is not the code's to change)
#@row old: present={a} definite={a} closed

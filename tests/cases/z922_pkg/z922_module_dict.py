"""A module's `__dict__` is its namespace: reads give its globals (a decorated function as
decorated), stores bind globals its functions see.

Expectations were written from the program text before running the engine."""
import plainmod

d = plainmod.__dict__
a = d["X"]                                   #@value a: exactly={1}
has = "f" in d                               #@value has: exactly={True}
r = d["f"](2)                                #@value r: exactly={30}
lab = d["f"].label                           #@value lab: exactly={"t"}
d["Z"] = 7
z = plainmod.Z                               #@value z: exactly={7}
gz = plainmod.g()                            #@value gz: exactly={7}


def via_function():
    try:
        return plainmod.__dict__["X"]
    except AttributeError:
        return "no dict"


vf = via_function()                          #@value vf: exactly={1}

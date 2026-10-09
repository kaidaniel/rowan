"""Hypothesis: `dict.update` with a literal dict, with keyword arguments, with another
variable, and inside a loop over known items each add exactly the right keys.

Expectations were written from the program text before running the engine."""

base = {"a": 1}
base.update({"b": 2})
base.update(c=3)
extra = {"d": 4}
base.update(extra)
_ = 0                                     #@row base: present={a,b,c,d} definite={a,b,c,d} closed
acc = {}
for part in [{"x": 1}, {"y": 2}]:
    acc.update(part)
_ = 0                                     #@row acc: present={x,y} definite={x,y} closed
merged = {**base, **acc}                  #@row merged: present={a,b,c,d,x,y} definite={a,b,c,d,x,y} closed
base["x"]                                 #@ missing x

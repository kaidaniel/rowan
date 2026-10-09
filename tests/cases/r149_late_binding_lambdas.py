"""Hypothesis: lambdas made in a comprehension close over the comprehension's variable, so
after the comprehension every one of them returns the last value; a default argument
(`k=k`) captures each value instead.

Expectations were written from the program text before running the engine."""

fns = [lambda: k for k in ("a", "b", "c")]
out = {}
for f in fns:
    out[f()] = True
_ = 0                                        #@row out: present={c} definite={c} closed
out["a"]                                     #@ missing a
fns2 = [lambda k=k: k for k in ("a", "b", "c")]
out2 = {f(): True for f in fns2}             #@row out2: present={a,b,c} definite={a,b,c} closed

"""Hypothesis: keys built from an f-string with a method call inside and a format spec, then
`.upper()`, are known.

Expectations were written from the program text before running the engine."""

d = {}
for i, w in enumerate(["Alpha", "beta"]):
    d[f"{w.lower()}_{i:02d}".upper()] = i
_ = 0                                        #@row d: present={ALPHA_00,BETA_01} definite={ALPHA_00,BETA_01} closed

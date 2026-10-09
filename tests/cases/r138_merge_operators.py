"""Hypothesis: `{**a, **b}`, `a | b` and `a |= b` merge rows exactly; `|` does not mutate
its left side, `|=` does; a conditional splat makes its keys possible only.

Expectations were written from the program text before running the engine."""

base = {"a": 1, "b": 2}
over = {"b": 20, "c": 30}
m1 = {**base, **over, "d": 4}                #@row m1: present={a,b,c,d} definite={a,b,c,d} closed
m2 = base | over                             #@row m2: present={a,b,c} definite={a,b,c} closed
base["c"]                                    #@ missing c
base |= {"e": 5}
_ = 0                                        #@row base: present={a,b,e} definite={a,b,e} closed
m3 = {**base, **(over if flag else {})}      #@row m3: present={a,b,c,e} definite={a,b,e} closed
m3["c"]                                      #@ possibly c
m4 = over | {"b": None}                      #@row m4: present={b,c} definite={b,c} closed

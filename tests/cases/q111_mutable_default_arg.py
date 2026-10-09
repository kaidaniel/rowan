"""Hypothesis: a mutable default argument is one object shared by every call that omits the
argument, so keys added by one call are present in the next; a call passing its own dict is
independent.

Expectations were written from the program text before running the engine."""


def add(key, bucket={}):
    bucket[key] = True
    return bucket


first = add("a")                          #@row first: present={a} definite={a} closed
second = add("b")                         #@row second: present={a,b} definite={a,b} closed
own = add("c", {})                        #@row own: present={c} definite={c} closed
_ = 0                                     #@row first: present={a,b} definite={a,b} closed
own["a"]                                  #@ missing a

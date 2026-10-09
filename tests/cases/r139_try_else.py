"""Hypothesis: `try/except/else/finally` around `int(text)` on a literal decides which of
the handler and the `else` runs: `int("12")` does not raise, `int("x")` raises ValueError;
on an unknown string both are possible.

Expectations were written from the program text before running the engine."""


def load(text):
    out = {"src": text}
    try:
        n = int(text)
    except ValueError:
        out["error"] = "nan"
    else:
        out["value"] = n
    finally:
        out["done"] = True
    return out


ok = load("12")                              #@row ok: present={src,value,done} definite={src,value,done} closed
bad = load("x")                              #@row bad: present={src,error,done} definite={src,error,done} closed
ok["error"]                                  #@ missing error
bad["value"]                                 #@ missing value
u = load(text_in)                            #@row u: present={src,value,error,done} definite={src,done} closed
u["value"]                                   #@ possibly value

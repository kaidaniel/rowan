"""Three curried lambdas building a dict from the captured values.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""

f = lambda a: lambda b: lambda c: {a: b, "c": c}
r = f("k")("v")(3)                           #@row r: present={k,c} definite={k,c} closed
g = f("j")
s = g("w")(4)                                #@row s: present={j,c} definite={j,c} closed

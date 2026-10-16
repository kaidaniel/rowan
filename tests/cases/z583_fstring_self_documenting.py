# `f"{x=}"` writes the expression's text, `=`, and the value's repr (its str with `!s`, the
# spec applied to it with one).
x = 10
s = "ab"
a = f"{x=}"                                  #@value a: exactly={"x=10"}
b = f"{s=}"                                  #@value b: exactly={"s='ab'"}
c = f"{x = }"                                #@value c: exactly={"x = 10"}
d = f"{x=:>5}"                               #@value d: exactly={"x=   10"}
e = f"{s=!s}"                                #@value e: exactly={"s=ab"}
h = f"{x + 1=}"                              #@value h: exactly={"x + 1=11"}

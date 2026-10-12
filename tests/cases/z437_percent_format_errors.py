"""Hypothesis: %-formatting and format() give exactly what Python gives, and where Python raises
the formatting is not guessed: `"%d" % "x"` raises TypeError (it was formatted as "x", and the
handler was dead), `format("ab", "d")` raises ValueError.

Expectations were written from the program text before running the engine."""


def pct(spec, arg):
    try:
        return spec % arg
    except TypeError:
        return "type error"


def fmt(v, spec):
    try:
        return format(v, spec)
    except ValueError:
        return "value error"


ok = "%s-%s" % ("a", "b")                    #@value ok: exactly={"a-b"}
pad = "%5.1f|" % 2.25                        #@value pad: exactly={"  2.2|"}
hx = "%x" % 255                              #@value hx: exactly={"ff"}
bad = pct("%d", "x")                         #@value bad: includes={"type error"}
good = pct("%d", 7)                          #@value good: exactly={"7"}
f1 = format(3.14159, "08.3f")                #@value f1: exactly={"0003.142"}
f2 = fmt("ab", "d")                          #@value f2: includes={"value error"}
f3 = fmt(42, ">5")                           #@value f3: exactly={"   42"}

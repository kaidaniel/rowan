# math / statistics / fractions / decimal on literals: values follow from the program text

import math
import statistics
from decimal import ROUND_HALF_UP, Decimal
from fractions import Fraction


def summarize(values):
    return {"mean": statistics.mean(values), "median": statistics.median(values), "max": max(values)}


def field_mean(rows, field):
    return statistics.fmean(r[field] for r in rows)


s = summarize([1, 3, 5, 7])                    #@row s: present={mean,median,max} definite={mean,median,max} closed
mean = s["mean"]                               #@value mean: exactly={4}
med = s["median"]                              #@value med: exactly={4.0}
mode = statistics.mode(["a", "b", "a"])        #@value mode: exactly={"a"}
r = math.sqrt(16)                              #@value r: exactly={4.0}
fl = math.floor(7.8)                           #@value fl: exactly={7}
ce = math.ceil(7.1)                            #@value ce: exactly={8}
g = math.gcd(12, 18)                           #@value g: exactly={6}
comb = math.comb(5, 2)                         #@value comb: exactly={10}
fr = Fraction(1, 3) + Fraction(1, 6)
half = fr == Fraction(1, 2)                    #@value half: exactly={True}
num = fr.numerator                             #@value num: exactly={1}
ds = str(Decimal("1.10") + Decimal("2.20"))    #@value ds: exactly={"3.30"}
q = str(Decimal("2.675").quantize(Decimal("0.01"), rounding=ROUND_HALF_UP))   #@value q: exactly={"2.68"}
isc = math.isclose(0.1 + 0.2, 0.3)             #@value isc: exactly={True}
fm = field_mean([{"x": 1}, {"x": 2, "y": 0}], "x")   #@value fm: exactly={1.5}
try:
    field_mean([{"y": 1}], "x")                #@ missing x
except KeyError:
    pass

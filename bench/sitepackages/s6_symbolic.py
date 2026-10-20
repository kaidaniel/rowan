"""Scenario 6: symbolic maths on sympy (+ mpmath), a package with a heavy import (hundreds of
modules, decorators and registries run at import time)."""

import sympy as sp
from sympy import Rational, Symbol, diff, expand, factor, integrate, solve


def quadratic_roots(a: int, b: int, c: int) -> list:
    x = Symbol("x")
    return solve(a * x**2 + b * x + c, x)


def calculus(n: int) -> dict:
    x = Symbol("x")
    poly = expand((x + 1) ** n)
    return {
        "poly": str(poly),
        "derivative": str(diff(poly, x)),
        "integral": str(integrate(poly, (x, 0, 1))),
        "factored": str(factor(poly)),
    }


def report() -> dict:
    roots = quadratic_roots(1, -3, 2)
    stats = calculus(3)
    half = Rational(1, 2) + Rational(1, 3)
    return {"roots": [str(r) for r in roots], "half": str(half), "pi": float(sp.pi.evalf(5)), **stats}


if __name__ == "__main__":
    out = report()
    for k, v in out.items():
        print(f"{k}: {v}")

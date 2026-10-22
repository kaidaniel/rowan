"""An unknown a unary operator or a call gives is named by the expression, as `n * 2` already
was: `-f` was `?`, and so were calls whose result had no name of its own (decimal's
`pow(10, self._exp, m)`). The names stay inside their function; a value with a type keeps it."""


def ratio(f):
    neg = -f
    #@general neg: unknown
    r = f.as_integer_ratio()
    #@general r: unknown
    h = f.hex()
    #@general h: unknown
    return neg, r, h


print(ratio(0.5))

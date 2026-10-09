"""Money helpers: the innermost module of the package."""

INFO = {"unit": "cent", "symbol": "EUR"}


def cents(amount):
    return int(round(amount * 100))


def fmt(c):
    return "%s %d.%02d" % (INFO["symbol"], c // 100, c % 100)

"""Small text helpers used for labels and keys."""


def slug(s):
    return s.lower().replace(" ", "-")


def money(cents, symbol="EUR"):
    return "%s %d.%02d" % (symbol, cents // 100, cents % 100)


def pad(s, width):
    return s + " " * (width - len(s))


def key_for(kind, ident):
    return kind + ":" + str(ident)


def parse_key(key):
    kind, _, ident = key.partition(":")
    return {"kind": kind, "ident": ident}

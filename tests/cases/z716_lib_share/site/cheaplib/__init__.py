"""A small fake installed package for z716_library_share.py: cheap to import and to call."""

VALUE = "cheap"


def double(n):
    return n * 2


def register(reg, name):
    reg[name] = True

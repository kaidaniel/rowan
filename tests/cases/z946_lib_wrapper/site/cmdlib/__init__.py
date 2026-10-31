"""A command registry in a followed package: its decorators wrap the program's functions with
functools.update_wrapper, which sets attributes on the wrapper and changes nothing else."""
from functools import update_wrapper

COMMANDS = {}


def command(f):
    def wrapper(**kwargs):
        return f(**kwargs)

    update_wrapper(wrapper, f)
    COMMANDS[f.__name__] = wrapper
    return f


def wrapped(f):
    def wrapper(*args):
        return f(*args)

    return update_wrapper(wrapper, f)

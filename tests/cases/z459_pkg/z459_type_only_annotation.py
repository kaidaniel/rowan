"""Hypothesis: a class imported only under `if TYPE_CHECKING:` (not imported when the
program runs) still types a parameter annotated with it, once its module is loaded: an
entry run of a function nobody calls gets an instance of it. (rich annotates with names it
imports only for type checkers.)

Expectations were written from the program text before running the engine."""

from typing import TYPE_CHECKING

import z459models

if TYPE_CHECKING:
    from z459models import User


def greet(u: "User"):
    return "hi " + u.name                    #@ ok name


def shout(u: "User"):
    return u.nickname                        #@ missing nickname


known = z459models.User("ada").name          #@value known: exactly={ada}

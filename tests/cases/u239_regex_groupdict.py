"""Hypothesis: `re.match(pattern, s).groupdict()` has the pattern's named groups as keys.

Expectations were written from the program text before running the engine."""

import re

m = re.match(r"(?P<user>\w+)@(?P<host>\w+)", "ann@example")
gd = m.groupdict()                           #@row gd: present={user,host} definite={user,host} closed
gd["port"]                                   #@ missing port

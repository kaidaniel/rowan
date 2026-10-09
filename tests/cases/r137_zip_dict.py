"""Hypothesis: `dict(zip(keys, values))` with known keys gives exactly the keys that zip
pairs up (zip stops at the shorter input); with values of unknown length every key is
possible and none definite.

Expectations were written from the program text before running the engine."""

KEYS = ["id", "name", "email"]
rec = dict(zip(KEYS, [1, "n", "e"]))         #@row rec: present={id,name,email} definite={id,name,email} closed
short = dict(zip(KEYS, [1, "n"]))            #@row short: present={id,name} definite={id,name} closed
short["email"]                               #@ missing email
pairs = list(zip(KEYS, [1, 2, 3]))
back = dict(pairs)                           #@row back: present={id,name,email} definite={id,name,email} closed
comp = {k: v for k, v in zip(KEYS, "abc")}   #@row comp: present={id,name,email} definite={id,name,email} closed
# Corrected on re-reading against design case d30 (`dict(zip(["id", "name"], values))` makes
# `id` ok): with values of unknown length the header/row idiom is assumed to pair every key.
loose = dict(zip(KEYS, row))                 #@row loose: present={id,name,email} definite={id,name,email} closed
loose["phone"]                               #@ missing phone

"""Hypothesis: keys computed from values of a known dict (inverse map, `len` of keys,
`enumerate` with `start=`) are known literals.

Expectations were written from the program text before running the engine."""

CODES = {"red": "r", "green": "g"}
inv = {v: k for k, v in CODES.items()}       #@row inv: present={r,g} definite={r,g} closed
inv["red"]                                   #@ missing red
by_len = {len(k): k for k in CODES}          #@row by_len: present={3,5} definite={3,5} closed
by_len[4]                                    #@ missing 4
numbered = {i: n for i, n in enumerate(CODES.values(), start=10)}  #@row numbered: present={10,11} definite={10,11} closed
numbered[0]                                  #@ missing 0

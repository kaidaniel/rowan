"""Hypothesis: integer keys computed by arithmetic (`i + 1`, `2 * i`) index the right
entries, and a missing integer key is reported.

Expectations were written from the program text before running the engine."""

slots = {1: {"n": "one"}, 2: {"n": "two"}, 4: {"n": "four"}}
i = 1
a = slots[i + 1]                          #@row a: present={n} definite={n} closed
b = slots[2 * i + 2]                      #@row b: present={n} definite={n} closed
slots[3]                                  #@ missing 3
slots[i * 4]                              #@ ok 4
slots[i - 1]                              #@ missing 0

"""Hypothesis: an imported module's `if __name__ == "__main__":` block does not run (its
`__name__` is its own name), its `else` does; the entry's does. rowan ran every module's
demo block (rich.live's demo drove much of rich).

Expectations were written from the program text before running the engine."""

import z456demo

ran = z456demo.STATE["ran"]                  #@value ran: exactly={False}
imp = z456demo.imported                      #@value imp: exactly={True}

if __name__ == "__main__":
    here = "main"
else:
    here = "imported"
h = here                                     #@value h: exactly={main}

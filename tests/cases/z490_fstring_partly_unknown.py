"""An f-string of a value that is a literal on some paths and unknown on others (an environment
variable with a default): its text is not only the literal's. rowan made `f"{mode}!"` and
`"{}!".format(mode)` exactly "dev!" (the field's unknown part was dropped).

Expectations were written from the program text before running the engine."""
import os

mode = os.environ.get("Z490_MODE", "dev")
f = f"{mode}!"                           #@value f: unknown
g = f"[{mode}]"                          #@value g: unknown
fixed = f"{'dev'}!"                      #@value fixed: exactly={"dev!"}
h = "{}!".format(mode)                   #@value h: unknown

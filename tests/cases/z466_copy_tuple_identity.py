"""Hypothesis: `copy.copy` of a tuple and `copy.deepcopy` of a tuple of immutable values
return the tuple itself (CPython's copy module), so copying a configuration dict many
times does not make a new tuple per copy (timm deep-copies each model's config: rowan made
48k tuple objects, and every value read from the configs held all of them).

Expectations were written from the program text before running the engine."""

import copy

MEAN = (0.485, 0.456, 0.406)
cfg = {"mean": MEAN, "size": (3, 224, 224), "layers": [1, 2]}
deep = copy.deepcopy(cfg)
same = deep["mean"] is MEAN                  #@value same: exactly={True}
other = deep["layers"] is cfg["layers"]      #@value other: exactly={False}
t = copy.copy(MEAN) is MEAN                  #@value t: exactly={True}
nested = ([1], 2)
n2 = copy.deepcopy(nested)
fresh = n2 is nested                         #@value fresh: exactly={False}
size = deep["size"][1]                       #@value size: exactly={224}

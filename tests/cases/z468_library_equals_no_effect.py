"""Hypothesis: a library object's comparing method changes nothing it is given: pandas'
`a.index.equals(b)` (rowan does not keep a frame's index as an object) does not make what
it was given uncertain. (sktime compares indexes and frames with `.equals`: 26k such calls,
each walking all that its arguments reach.)

Expectations were written from the program text before running the engine."""

import pandas as pd

frame = pd.DataFrame({"x": [1, 2]})
expected = {"rows": 2, "strict": True}
same = frame.index.equals(expected)
strict = expected["strict"]                  #@ ok strict
#@value strict: exactly={True}

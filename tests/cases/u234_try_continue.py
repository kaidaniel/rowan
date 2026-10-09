"""Hypothesis: in a loop over known rows, `try: v = r["v"] except KeyError: continue` skips
exactly the rows without "v".

Expectations were written from the program text before running the engine."""

ROWS = [{"id": 1, "v": 2}, {"id": 2}, {"id": 3, "v": 0}]
out = {}
for r in ROWS:
    try:
        v = r["v"]
    except KeyError:
        continue
    out[r["id"]] = v
_ = 0                                        #@row out: present={1,3} definite={1,3} closed
out[2]                                       #@ missing 2

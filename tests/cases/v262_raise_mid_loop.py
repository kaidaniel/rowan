"""Hypothesis: an exception raised in the second iteration of a loop over known items, caught
outside the loop, leaves exactly the first iteration's store.

Expectations were written from the program text before running the engine."""

ITEMS = [{"id": "a"}, {"id": "b", "bad": True}, {"id": "c"}]
done = {}
try:
    for it in ITEMS:
        if it.get("bad"):
            raise ValueError(it["id"])
        done[it["id"]] = True
except ValueError:
    pass
_ = 0                                        #@row done: present={a} definite={a} closed

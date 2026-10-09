"""Hypothesis: a walrus in a comprehension filter binds the value for the element, and a
dict comprehension of list comprehensions keeps the right keys and positions.

Expectations were written from the program text before running the engine."""

ROWS = [{"id": 1, "tags": ["x", "y"]}, {"id": 2}, {"id": 3, "tags": []}]

tagged = {r["id"]: t for r in ROWS if (t := r.get("tags"))}   #@row tagged: present={1} definite={1} closed
by_id = {r["id"]: [{"tag": t, "owner": r["id"]} for t in r.get("tags", [])] for r in ROWS}  #@row by_id: present={1,2,3} definite={1,2,3} closed
first_tag = by_id[1][0]                   #@row first_tag: present={tag,owner} definite={tag,owner} closed
tagged[2]                                 #@ missing 2

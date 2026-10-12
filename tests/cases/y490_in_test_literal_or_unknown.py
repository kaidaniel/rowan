"""`n in visited` where `n` is a literal or an unknown value (a worklist that starts from a
known item and then takes items from input): on the branch where the test is false, `n` may be
any other key, so `visited[n] = …` adds unknown keys. The test narrowed by the literal alone,
`"s" not in visited` was impossible once "s" was stored, and the branch died: `visited` read
as closed with only "s"."""
import json
import sys

more = json.loads(sys.argv[1] if len(sys.argv) > 1 else "[]")
visited = {}
stack = ["s"]
while stack:
    n = stack.pop()
    if n in visited:
        continue
    visited[n] = 1
    for dst in more:
        stack.append(dst)
_ = 0  #@row visited: present={s} definite={s} open
visited["s"]  #@ ok s

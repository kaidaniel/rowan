# A slice of an input sequence holds the input's own elements: an element read through the
# slice (by a loop, by an index) reads one of the input's elements, not an element of one.
import json
import os

TEXT = os.environ.get("ROWAN_CASE_ROWS", '[{"amount": 1}, {"amount": 2, "name": "b"}]')


def total(rows):
    t = 0
    for r in rows[1:]:
        t += r["amount"]
    return t


def second_name(users):
    rest = users[1:]
    return rest[0]["name"]


def tail_names(users):
    return [u["name"] for u in users[1:][::-1]]
#@schema param tail_names.users['*']: always={name} only={name}


data = json.loads(TEXT)
t = total(data)
n = second_name(data)
#@schema json.loads@L26['*']: always={amount,name} only={amount,name}

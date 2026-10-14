"""A library call that changes the list it is given (`random.shuffle`) or one rowan cannot see
leaves its length and positions unknown. `random.*` was taken as having no effect, and unknown
calls changed only the rows of records, so `objs[0]` read as the first object for sure and
`first` lacked the second object's keys."""
import random


class Obj:
    def __init__(self, payload):
        self.payload = payload


objs = [Obj({"kind": "f", "qualname": "x"}), Obj({"kind": "d", "size": 1})]
random.shuffle(objs)
first = objs[0].payload                   #@row first: present={kind,qualname,size}
#@row first: definite={kind} closed
first.get("size")

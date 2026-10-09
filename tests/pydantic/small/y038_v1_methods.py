# v1 spellings that still work in v2 (deprecated): .dict() / .json() / parse_obj / parse_raw / copy(update=) / __fields__ / construct / schema
import json
import warnings

from pydantic import BaseModel, ValidationError

warnings.simplefilter("ignore", DeprecationWarning)


class Order(BaseModel):
    id: int
    item: str
    qty: int = 1
    note: str | None = None


def legacy(d):
    o = Order.parse_obj(d)                       #@ possibly item
    #@row o: present={id,item,qty,note} definite={id,item,qty,note} closed
    plain = o.dict(exclude_none=True)
    #@row plain: present={id,item,qty,note} definite={id,item,qty} closed
    text = o.json(include={"id", "qty"})
    back = json.loads(text)
    #@row back: present={id,qty} definite={id,qty} closed
    bumped = o.copy(update={"qty": o.qty + 1})
    #@row bumped: present={id,item,qty,note} definite={id,item,qty,note} closed
    return bumped.dict()


r = Order.parse_raw('{"id": 1, "item": "pen"}')
#@row r: present={id,item,qty,note} definite={id,item,qty,note} closed
q = r.qty
#@value q: exactly={1}
fields = Order.__fields__
#@row fields: present={id,item,qty,note} definite={id,item,qty,note} closed
c = Order.construct(id=2)
#@row c: present={id,qty,note} definite={id,qty,note} absent={item} closed
props = Order.schema()["properties"]
#@row props: present={id,item,qty,note} definite={id,item,qty,note} closed
cp = r.copy(update={"qty": 5}).dict()
#@row cp: present={id,item,qty,note} definite={id,item,qty,note} closed
cq = cp["qty"]
#@value cq: exactly={5}

if __name__ == "__main__":
    legacy({"id": 1, "item": "a"})
    legacy({"id": 2, "item": "b", "qty": 3, "note": "n"})
    try:
        legacy({"id": 3})
    except ValidationError:
        pass

#@schema param legacy.d: always={id,item} only={id,item}

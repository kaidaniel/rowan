# list[Model] / dict[str, Model] fields: elements are validated models; dumps are lists / dicts of dicts
from pydantic import BaseModel, ValidationError


class Line(BaseModel):
    sku: str
    qty: int = 1


class Cart(BaseModel):
    owner: str
    lines: list[Line] = []
    by_sku: dict[str, Line] = {}


def total_qty(payload):
    cart = Cart.model_validate(payload)          #@ possibly sku
    total = 0
    for ln in cart.lines:
        total += ln.qty                          #@ ok qty
        #@row ln: present={sku,qty} definite={sku,qty} closed
    for key, ln2 in cart.by_sku.items():
        s = ln2.sku                              #@ ok sku
    d = cart.model_dump()
    #@row d: present={owner,lines,by_sku} definite={owner,lines,by_sku} closed
    for ld in d["lines"]:
        #@row ld: present={sku,qty} definite={sku,qty} closed
        total += ld["qty"]                       #@ ok qty
    return total


c = Cart(owner="o", lines=[{"sku": "a"}, Line(sku="b", qty=3)])
n = len(c.lines)
#@value n: exactly={2}
first = c.lines[0]
#@row first: present={sku,qty} definite={sku,qty} closed
q = first.qty
#@value q: exactly={1}

if __name__ == "__main__":
    total_qty({"owner": "a"})
    total_qty({"owner": "b", "lines": [{"sku": "x", "qty": 2}, {"sku": "y"}], "by_sku": {"z": {"sku": "z"}}})
    try:
        total_qty({"owner": "c", "lines": [{"qty": 2}]})
    except ValidationError:
        pass

#@schema param total_qty.payload: always={owner} only={owner}
#@schema param total_qty.payload['lines']['*']: always={sku} only={sku}
#@schema param total_qty.payload['by_sku']['*']: always={sku} only={sku}

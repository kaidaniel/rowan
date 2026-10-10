# Stock ledger: dataclasses built from csv.DictReader rows (__post_init__, default_factory, Optional field), mutated by a Ledger's patch method (literal patches and JSON patches from os.environ), asdict | merge, replace, fields(), round trip through csv.DictWriter / DictReader
"""Hypothesis: a dataclass instance built from a parsed row and mutated by methods of another
class keeps exactly its fields (closed), also when the patches come from input: the patch only
assigns known fields, so only the values become unknown. asdict merged with a literal dict adds
one key; a text written by DictWriter with literal fieldnames reads back as closed rows."""
import csv
import io
import json
import os
from dataclasses import asdict, dataclass, field, fields, replace
from decimal import Decimal
from typing import Optional

STOCK_CSV = """sku,name,qty,price
a1,Bolt,40,0.25
b2,Nut,100,0.10
c3,Washer,0,0.05
"""


@dataclass
class Item:
    sku: str
    name: str
    qty: int
    price: Decimal
    tags: list = field(default_factory=list)
    location: Optional[str] = None

    def __post_init__(self):
        self.sku = self.sku.upper()


def parse_row(row):
    return Item(row["sku"], row["name"].strip(), int(row["qty"]), Decimal(row["price"]))
#@schema param parse_row.row: always={sku,name,qty,price} only={sku,name,qty,price}


class Ledger:
    def __init__(self, text):
        self.items = {}
        for row in csv.DictReader(io.StringIO(text)):
            item = parse_row(row)
            self.items[item.sku] = item
        self.log = []

    def apply(self, patch):
        item = self.items[patch["sku"]]
        op = patch["op"]                         #@ ok op ; input op
        if op == "add":
            item.qty += int(patch["n"])
        elif op == "remove":
            item.qty = max(0, item.qty - int(patch["n"]))
        elif op == "tag":
            item.tags.append(patch["tag"])
        elif op == "move":
            item.location = patch["to"]
        else:
            raise ValueError(op)
        self.log.append((op, item.sku))
        #@row item: present={sku,name,qty,price,tags,location} definite={sku,name,qty,price,tags,location} closed
        return item

    def export(self):
        out = []
        for item in self.items.values():
            rec = asdict(item) | {"value": str(item.qty * item.price)}
            #@row rec: present={sku,name,qty,price,tags,location,value} definite={sku,name,qty,price,tags,location,value} closed
            out.append(rec)
        return out


def to_csv(records):
    buf = io.StringIO()
    w = csv.DictWriter(buf, fieldnames=["sku", "qty", "value"], extrasaction="ignore")
    w.writeheader()
    w.writerows(records)
    return buf.getvalue()


if __name__ == "__main__":
    led = Ledger(STOCK_CSV)
    skus = tuple(led.items)                      #@value skus: exactly={("A1","B2","C3")}
    a1 = led.apply({"sku": "A1", "op": "remove", "n": "15"})
    #@row a1: present={sku,name,qty,price,tags,location} definite={sku,name,qty,price,tags,location} closed
    q = a1.qty                                   #@value q: exactly={25}
    led.apply({"sku": "C3", "op": "add", "n": 12})
    led.apply({"sku": "B2", "op": "move", "to": "shelf-4"})
    loc = led.items["B2"].location               #@value loc: exactly={"shelf-4"}
    noloc = led.items["A1"].location             #@value noloc: exactly={None}
    led.apply({"sku": "B2", "op": "tag", "tag": "bulk"})
    tags = tuple(led.items["B2"].tags)           #@value tags: exactly={("bulk",)}
    try:
        led.apply({"sku": "A1", "op": "melt"})
    except ValueError:
        pass
    nlog = len(led.log)                          #@value nlog: exactly={4}
    names = tuple(f.name for f in fields(Item))  #@value names: exactly={("sku","name","qty","price","tags","location")}
    zero = replace(led.items["A1"], qty=0)
    #@row zero: present={sku,name,qty,price,tags,location} definite={sku,name,qty,price,tags,location} closed
    zsku = zero.sku                              #@value zsku: exactly={A1}

    recs = led.export()
    c3 = recs[2]
    #@row c3: present={sku,name,qty,price,tags,location,value} definite={sku,name,qty,price,tags,location,value} closed
    v3 = c3["value"]                             #@value v3: exactly={"0.60"}
    text = to_csv(recs)
    back = list(csv.DictReader(io.StringIO(text)))
    b0 = back[0]
    #@row b0: present={sku,qty,value} definite={sku,qty,value} absent={name,price} closed
    bq = b0["qty"]                               #@value bq: exactly={"25"}

    patches = json.loads(os.environ.get("STOCK_PATCHES", '[{"sku": "B2", "op": "remove", "n": 30}, {"sku": "C3", "op": "move", "to": "bin-9"}]'))
    #@schema json.loads@L113['*']: always={sku,op} sometimes={n,tag,to} only={sku,op,n,tag,to}
    for pt in patches:
        led.apply(pt)
    b2 = led.items["B2"]
    #@row b2: present={sku,name,qty,price,tags,location} definite={sku,name,qty,price,tags,location} closed
    recs2 = led.export()
    r0 = recs2[0]
    #@row r0: present={sku,name,qty,price,tags,location,value} definite={sku,name,qty,price,tags,location,value} closed

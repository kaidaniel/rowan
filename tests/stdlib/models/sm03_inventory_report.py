"""Model: an inventory with stock movements and a restock report. Raw records → dataclass items
(field defaults, default_factory, __post_init__ adding attributes, extra attributes set later that
dataclasses.replace / asdict do not carry), an Enum for categories (Enum(value), Enum['NAME'],
.name / .value as keys), namedtuple movements with a default, OrderedDict / deque / defaultdict /
Counter for the bookkeeping, and report records built from all of them."""
from collections import Counter, OrderedDict, defaultdict, deque, namedtuple
from dataclasses import asdict, dataclass, field, fields, replace
from enum import Enum


class Category(Enum):
    FOOD = "food"
    TOOL = "tool"
    TOY = "toy"


@dataclass
class Item:
    sku: str
    name: str
    category: Category
    qty: int = 0
    price: float = 0.0
    tags: list = field(default_factory=list)

    def __post_init__(self):
        if self.qty < 0:
            raise ValueError("negative stock")
        if self.qty == 0:
            self.out_of_stock = True


Movement = namedtuple("Movement", "sku delta reason", defaults=["adjust"])

RAW = [
    {"sku": "A1", "name": "apple", "category": "food", "qty": 10, "price": 0.5},
    {"sku": "T1", "name": "hammer", "category": "tool", "qty": 0, "price": 12.0, "tags": ["heavy"]},
    {"sku": "Y1", "name": "yoyo", "category": "toy", "qty": 3, "price": 2.0, "discount": 0.1},
    {"sku": "B2", "name": "bread", "category": "food", "price": 1.5},
]


def parse_item(raw):
    item = Item(
        sku=raw["sku"],
        name=raw["name"],
        category=Category(raw["category"]),
        qty=raw.get("qty", 0),
        price=raw.get("price", 0.0),
        tags=list(raw.get("tags", [])),
    )
    if "discount" in raw:
        item.discount = raw["discount"]          #@ ok discount
    _ = 0                                        #@row item: present={sku,name,category,qty,price,tags,out_of_stock,discount} definite={sku,name,category,qty,price,tags} closed
    return item
#@schema param parse_item.raw: always={sku,name,category} only={sku,name,category}


class Inventory:
    def __init__(self):
        self.items = OrderedDict()
        self.log = deque(maxlen=3)
        self.rejected = Counter()

    def add(self, item):
        self.items[item.sku] = item

    def apply(self, mv):
        if mv.sku not in self.items:
            self.rejected[mv.reason] += 1
            return None
        item = self.items[mv.sku]
        try:
            new = replace(item, qty=item.qty + mv.delta)
        except ValueError:
            self.rejected["negative"] += 1
            return None
        _ = 0                                    #@row new: present={sku,name,category,qty,price,tags,out_of_stock} definite={sku,name,category,qty,price,tags} absent={discount} closed
        self.items[mv.sku] = new
        self.items.move_to_end(mv.sku)
        self.log.append(mv)
        return new


def load(raws):
    inv = Inventory()
    bad = []
    for raw in raws:
        try:
            inv.add(parse_item(raw))
        except ValueError:
            bad.append(raw.get("sku"))
    _ = 0                                        #@row inv: present={items,log,rejected} definite={items,log,rejected} closed
    return inv, tuple(bad)


def by_category(inv):
    groups = defaultdict(list)
    for it in inv.items.values():
        groups[it.category.value].append(it.sku)
    return dict(groups)


def stock_value(inv):
    c = Counter()
    for it in inv.items.values():
        c[it.category.name] += round(it.qty * it.price, 2)
    return c


def restock(inv, threshold):
    out = []
    for it in inv.items.values():
        if it.qty < threshold:
            rec = asdict(it) | {"need": threshold - it.qty}
            _ = 0                                #@row rec: present={sku,name,category,qty,price,tags,need} definite={sku,name,category,qty,price,tags,need} absent={out_of_stock,discount} closed
            out.append(rec)
    return out


def card(rec):
    line = f"{rec['sku']}: {rec['name']}"        #@ ok sku ; ok name
    if "need" in rec:
        line += f" (+{rec['need']})"             #@ ok need
    return line
#@schema param card.rec: always={sku,name} only={sku,name}


def report(inv, threshold):
    cats = by_category(inv)                      #@row cats: present={food,tool,toy} definite={food,tool,toy} closed
    value = stock_value(inv)                     #@row value: present={FOOD,TOOL,TOY} definite={FOOD,TOOL,TOY} closed
    todo = restock(inv, threshold)
    recent = [m._asdict() for m in inv.log]
    for r in recent:
        _ = 0                                    #@row r: present={sku,delta,reason} definite={sku,delta,reason} closed
    doc = {
        "categories": cats,
        "value": dict(value),
        "restock": todo,
        "cards": [card(r) for r in todo],
        "recent": recent,
    }
    if inv.rejected:
        doc["rejected"] = dict(inv.rejected)
    _ = 0                                        #@row doc: present={categories,value,restock,cards,recent,rejected} definite={categories,value,restock,cards,recent} closed
    return doc


if __name__ == "__main__":
    inv, bad = load(RAW + [{"sku": "Z9", "name": "ghost", "category": "toy", "qty": -1}])
    #@value bad: exactly={("Z9",)}
    try:
        parse_item({"sku": "G1", "name": "gizmo", "category": "gadget"})
    except ValueError:
        pass
    order = tuple(inv.items)                     #@value order: exactly={("A1","T1","Y1","B2")}
    y = inv.items["Y1"]                          #@row y: present={sku,name,category,qty,price,tags,discount} definite={sku,name,category,qty,price,tags,discount} closed
    t = inv.items["T1"]
    t.out_of_stock                               #@ ok out_of_stock
    r0 = report(inv, 5)
    inv.apply(Movement("Y1", 4, "delivery"))
    inv.apply(Movement("A1", -10))
    inv.apply(Movement("T1", -1, "sale"))
    inv.apply(Movement("Q7", 1))
    y2 = inv.items["Y1"]
    try:
        y2.discount                              #@ missing discount
    except AttributeError:
        pass
    a = inv.items["A1"]                          #@row a: present={sku,name,category,qty,price,tags,out_of_stock} definite={sku,name,category,qty,price,tags,out_of_stock} closed
    order2 = tuple(inv.items)                    #@value order2: exactly={("T1","B2","Y1","A1")}
    rej = dict(inv.rejected)                     #@row rej: present={negative,adjust} definite={negative,adjust} closed
    last = inv.log[-1]                           #@row last: present={sku,delta,reason} definite={sku,delta,reason} closed
    why = last.reason                            #@value why: exactly={adjust}
    r1 = report(inv, 5)
    cards = r1["cards"]
    n = len(cards)                               #@value n: exactly={3}
    names = tuple(f.name for f in fields(Item))  #@value names: exactly={("sku","name","category","qty","price","tags")}
    food = Category["FOOD"].value                #@value food: exactly={food}
    try:
        Category["GADGET"]                       #@ missing GADGET
    except KeyError:
        pass
    card({"sku": "X", "name": "x", "extra": 1})

"""A warehouse holding Item objects in a dict keyed by SKU; methods mutate item attributes,
reserve stock and build report dicts of lists.

Expectations were written from the program text before running the engine."""


class Item:
    def __init__(self, sku, qty, price):
        self.sku = sku
        self.qty = qty
        self.price = price
        self.reserved = 0

    def reserve(self, n):
        if n > self.qty - self.reserved:
            self.shortfall = n - (self.qty - self.reserved)
            return False
        self.reserved += n
        return True


class Warehouse:
    def __init__(self):
        self.items = {}
        self.log = []

    def add(self, sku, qty, price):
        it = Item(sku, qty, price)
        self.items[sku] = it
        return it

    def restock(self, sku, n):
        it = self.items[sku]
        it.qty += n
        it.last_restock = n
        self.log.append({"op": "restock", "sku": sku, "n": n})

    def report(self):
        low = []
        value = 0
        for sku, it in self.items.items():
            value += it.qty * it.price
            if it.qty < 5:
                low.append({"sku": sku, "qty": it.qty})
        return {"value": value, "low": low, "count": len(self.items)}


w = Warehouse()
a = w.add("a", 10, 2.0)                   #@row a: present={sku,qty,price,reserved} definite={sku,qty,price,reserved} closed
b = w.add("b", 1, 5.0)
items = w.items                           #@row items: present={a,b} definite={a,b} closed
w.restock("a", 5)
_ = 0                                     #@row a: present={sku,qty,price,reserved,last_restock} definite={sku,qty,price,reserved,last_restock} closed
_ = 0                                     #@row b: present={sku,qty,price,reserved} definite={sku,qty,price,reserved} closed
ok = a.reserve(3)
# The comparison is open: the shortfall attribute exists on one path only.
_ = 0                                     #@row a: present={sku,qty,price,reserved,last_restock,shortfall} definite={sku,qty,price,reserved,last_restock} closed
rep = w.report()                          #@row rep: present={value,low,count} definite={value,low,count} closed
entry = w.log[0]                          #@row entry: present={op,sku,n} definite={op,sku,n} closed
low0 = rep["low"][0]                      #@row low0: present={sku,qty} definite={sku,qty} closed
b.shortfall                               #@ missing shortfall
a.last_restock                            #@ ok last_restock

"""An attribute an instance of a user class surely lacks (all its attributes known, no class
attribute of that name, no `__getattr__`) raises AttributeError on every path, also inside a
callee whose caller handles it: the store the call would feed never happens. It was taken as
only possibly raising, so the handler's path and a path with the store both went on."""


class Item:
    def __init__(self):
        self.name = "x"


def sku_of(item):
    return item.sku  #@ missing sku


def create_item(item):
    s = {}
    try:
        s["sku"] = sku_of(item)
    except AttributeError:
        s["fallback"] = True
    _ = 0  #@row s: present={fallback} definite={fallback} closed
    return s


r = create_item(Item())
#@row r: present={fallback} definite={fallback} closed

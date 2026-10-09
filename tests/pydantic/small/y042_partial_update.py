# PATCH semantics: a patch model with all-optional fields, model_dump(exclude_unset=True) as the change set, model_copy(update=) on the stored record
from pydantic import BaseModel, ValidationError


class Item(BaseModel):
    name: str
    price: float
    tags: list[str] = []


class ItemPatch(BaseModel):
    name: str | None = None
    price: float | None = None
    tags: list[str] | None = None


def patch_item(stored, body):
    p = ItemPatch.model_validate(body)
    changes = p.model_dump(exclude_unset=True)
    #@row changes: present={name,price,tags} absent={sku} closed
    current = Item.model_validate(stored)        #@ possibly price
    updated = current.model_copy(update=changes)
    #@row updated: present={name,price,tags} definite={name,price,tags} closed
    out = updated.model_dump()
    #@row out: present={name,price,tags} definite={name,price,tags} closed
    return out


r1 = patch_item({"name": "a", "price": 1.0}, {"price": 2})
p1 = r1["price"]
#@value p1: exactly={2.0}
n1 = r1["name"]
#@value n1: exactly={"a"}
r2 = patch_item({"name": "b", "price": 3.0, "tags": ["x"]}, {})
t2 = r2["tags"][0]
#@value t2: exactly={"x"}

if __name__ == "__main__":
    patch_item({"name": "c", "price": 4}, {"name": "d", "sku": "s"})
    patch_item({"name": "e", "price": 5}, {"tags": None})
    try:
        patch_item({"name": "f"}, {"price": 1})
    except ValidationError:
        pass

#@schema param patch_item.stored: always={name,price} only={name,price}
#@schema param patch_item.body: only={}

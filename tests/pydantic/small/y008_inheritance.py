# inherited fields, overridden defaults and types, a required field given a default in a subclass; one loader over several classes
from pydantic import BaseModel, ValidationError


class Base(BaseModel):
    id: int
    created: str = "now"


class Named(Base):
    name: str


class Product(Named):
    created: str = "later"
    price: float


class Loose(Named):
    name: str = "anon"


def load(cls, d):
    obj = cls.model_validate(d)
    #@row obj: present={id,created,name,price} definite={id,created} closed
    return obj


def loose(d):
    lo = Loose(**d)
    #@row lo: present={id,created,name} definite={id,created,name} absent={price} closed
    return lo.name                               #@ ok name


def price_of(obj):
    return obj.price                             #@ possibly price


p = Product(id=1, name="w", price=2.0)
#@row p: present={id,created,name,price} definite={id,created,name,price} closed
cr = p.created
#@value cr: exactly={"later"}
b = Base(id=2)
#@row b: present={id,created} definite={id,created} absent={name,price} closed
cb = b.created
#@value cb: exactly={"now"}

if __name__ == "__main__":
    for cls, d in [(Base, {"id": 1}), (Named, {"id": 2, "name": "n"}), (Product, {"id": 3, "name": "p", "price": 1.5})]:
        obj = load(cls, d)
        try:
            price_of(obj)
        except AttributeError:
            pass
    loose({"id": 1})
    loose({"id": 2, "name": "x", "price": 3})
    try:
        loose({"name": "y"})
    except ValidationError:
        pass

#@schema param loose.d: always={id} only={id}

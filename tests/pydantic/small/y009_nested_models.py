# nested models: validation from nested dicts, nested attribute rows, model_dump turns nested models into nested dicts
from pydantic import BaseModel, ValidationError


class Address(BaseModel):
    street: str
    city: str
    zip: str | None = None


class Person(BaseModel):
    name: str
    address: Address
    billing: Address | None = None


def register(payload):
    p = Person.model_validate(payload)           #@ possibly city
    #@row p: present={name,address,billing} definite={name,address,billing} closed
    a = p.address
    #@row a: present={street,city,zip} definite={street,city,zip} closed
    town = a.city                                #@ ok city
    if p.billing is not None:
        bill_city = p.billing.city               #@ ok city
    d = p.model_dump()
    #@row d: present={name,address,billing} definite={name,address,billing} closed
    da = d["address"]                            #@ ok address
    #@row da: present={street,city,zip} definite={street,city,zip} closed
    return da["city"]                            #@ ok city


home = Person(name="a", address=Address(street="s", city="c"), billing={"street": "t", "city": "d", "zip": "1"})
bz = home.billing.zip
#@value bz: exactly={"1"}
hz = home.address.zip
#@value hz: exactly={None}

if __name__ == "__main__":
    register({"name": "a", "address": {"street": "s", "city": "c"}})
    register({"name": "b", "address": {"street": "s", "city": "c", "zip": "9"}, "billing": {"street": "x", "city": "y"}})
    try:
        register({"name": "c", "address": {"street": "s"}})
    except ValidationError:
        pass

#@schema param register.payload: always={name,address} only={name,address}
#@schema param register.payload['address']: always={street,city} only={street,city}
#@schema param register.payload['billing']: always={street,city} only={street,city}

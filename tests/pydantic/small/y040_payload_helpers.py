# dict payloads validated by a helper called from several functions; one merges a patch into a dumped model and validates again
from pydantic import BaseModel, ValidationError


class Address(BaseModel):
    line1: str
    city: str
    country: str = "DE"


class Customer(BaseModel):
    name: str
    email: str = ""
    address: Address


def check_address(raw):
    a = Address.model_validate(raw)              #@ possibly city
    #@row a: present={line1,city,country} definite={line1,city,country} closed
    return a


def create_customer(payload):
    addr = check_address(payload["address"])     #@ ok address
    c = Customer(name=payload["name"], email=payload.get("email", ""), address=addr)   #@ possibly name
    #@row c: present={name,email,address} definite={name,email,address} closed
    return c


def update_address(customer, patch):
    merged = {**customer.address.model_dump(), **patch}
    #@row merged: present={line1,city,country,zip} definite={line1,city,country} closed
    customer.address = check_address(merged)
    return customer


if __name__ == "__main__":
    c1 = create_customer({"name": "a", "address": {"line1": "l", "city": "c"}})
    c2 = create_customer({"name": "b", "email": "e", "address": {"line1": "m", "city": "d", "country": "FR"}})
    update_address(c1, {"city": "x"})
    update_address(c2, {"zip": "1"})
    for bad in ({"name": "c", "address": {"line1": "n"}}, {"address": {"line1": "o", "city": "e"}}):
        try:
            create_customer(bad)
        except (KeyError, ValidationError):
            pass

#@schema param create_customer.payload: always={name,address} only={name,address}
#@schema param create_customer.payload['address']: always={line1,city} only={line1,city}
#@schema param check_address.raw: always={line1,city} only={line1,city}

# A raw dict body validated by hand inside helpers: Address(**raw) / Contact.model_validate(raw) — missing fields raise ValidationError
from typing import Any, Dict, Optional

from fastapi import FastAPI
from fastapi.testclient import TestClient
from pydantic import BaseModel, ValidationError

app = FastAPI()


class Address(BaseModel):
    street: str
    city: str
    zip: str = "00000"


class Contact(BaseModel):
    name: str
    phone: Optional[str] = None


def parse_address(raw):
    addr = Address(**raw)                            #@ input street ; input city
    #@row addr: present={street,city,zip} definite={street,city,zip} closed
    return addr


def parse_contact(raw):
    c = Contact.model_validate(raw)                  #@ input name
    #@row c: present={name,phone} definite={name,phone} closed
    return c


@app.post("/register")
def register(body: Dict[str, Any]):
    contact = parse_contact(body)
    addr = parse_address(body["address"])            #@ input address
    rec = {**contact.model_dump(), "address": addr.model_dump()}
    #@row rec: present={name,phone,address} definite={name,phone,address} closed
    return rec
#@schema param register.body: always={name,address} only={name,address}
#@schema param register.body['address']: always={street,city} only={street,city}


if __name__ == "__main__":
    client = TestClient(app)
    bodies = [
        {"name": "ann", "address": {"street": "Main 1", "city": "Oslo"}},
        {"name": "bob", "phone": "123", "address": {"street": "B", "city": "Rome", "zip": "9", "floor": 2}, "x": 1},
        {"name": "cyd", "address": {"city": "Rome"}},
        {"name": "dan"},
        {"address": {"street": "a", "city": "b"}},
    ]
    for b in bodies:
        try:
            client.post("/register", json=b)
        except (KeyError, ValidationError):
            pass

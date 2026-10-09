# Optional[X] / X | None fields: with a None default they are not required; exclude_none drops the ones that are None
from typing import Optional

from pydantic import BaseModel, ValidationError


class Contact(BaseModel):
    email: str
    phone: Optional[str] = None
    nick: str | None = None
    fax: Optional[str]          # Optional without a default is still required (v2)


def build(d):
    c = Contact.model_validate(d)                #@ possibly fax
    #@row c: present={email,phone,nick,fax} definite={email,phone,nick,fax} closed
    out = c.model_dump(exclude_none=True)
    #@row out: present={email,phone,nick,fax} definite={email} closed
    return out


def has_phone(d):
    out = build(d)
    return "phone" in out


c0 = Contact(email="a@x", fax=None)
#@row c0: present={email,phone,nick,fax} definite={email,phone,nick,fax} closed
p = c0.phone
#@value p: exactly={None}
d0 = c0.model_dump(exclude_none=True)
#@row d0: present={email} definite={email} closed
d1 = Contact(email="b@x", nick="bee", fax="1").model_dump(exclude_none=True)
#@row d1: present={email,nick,fax} definite={email,nick,fax} absent={phone} closed

if __name__ == "__main__":
    build({"email": "a", "fax": None})
    has_phone({"email": "a", "phone": "1", "fax": "2"})
    build({"email": "a", "nick": None, "fax": "f", "other": 1})
    try:
        build({"email": "a"})
    except ValidationError:
        pass

#@schema param build.d: always={email,fax} only={email,fax}

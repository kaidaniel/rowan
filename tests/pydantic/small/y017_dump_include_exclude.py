# model_dump(include= / exclude=) with sets and the nested dict form; a field set chosen by the caller
from pydantic import BaseModel


class Addr(BaseModel):
    city: str
    zip: str
    geo: str = "?"


class User(BaseModel):
    id: int
    name: str
    secret: str
    addr: Addr
    tags: list[str] = []


def public(u, fields):
    out = u.model_dump(include=fields)
    #@row out: present={id,name} definite={id} absent={secret,addr,tags} closed
    return out


u = User(id=1, name="n", secret="s", addr=Addr(city="c", zip="z"))
a = u.model_dump(include={"id", "name"})
#@row a: present={id,name} definite={id,name} closed
b = u.model_dump(exclude={"secret"})
#@row b: present={id,name,addr,tags} definite={id,name,addr,tags} absent={secret} closed
c = u.model_dump(include={"id": True, "addr": {"city"}})
#@row c: present={id,addr} definite={id,addr} closed
ca = c["addr"]                                   #@ ok addr
#@row ca: present={city} definite={city} absent={zip,geo} closed
d = u.model_dump(exclude={"addr": {"zip", "geo"}, "secret": True})
#@row d: present={id,name,addr,tags} definite={id,name,addr,tags} closed
da = d["addr"]
#@row da: present={city} definite={city} closed
e = u.model_dump(include={"id", "nope"})
#@row e: present={id} definite={id} absent={nope} closed

if __name__ == "__main__":
    public(u, {"id"})
    public(u, {"id", "name"})

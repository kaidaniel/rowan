# field declarations: required fields, plain defaults, Field(default=…); an instance's attributes are exactly its fields
from pydantic import BaseModel, Field, ValidationError


class User(BaseModel):
    id: int
    name: str
    role: str = "member"
    active: bool = Field(default=True)
    score: float = Field(0.0, ge=0)


def make(payload):
    u = User(**payload)                          #@ possibly name
    #@row u: present={id,name,role,active,score} definite={id,name,role,active,score} closed
    label = f"{u.name}:{u.role}"                 #@ ok name ; ok role
    return u


def email_of(u):
    return u.email                               #@ missing email


admin = User(id=1, name="ann", role="admin")
#@row admin: present={id,name,role,active,score} definite={id,name,role,active,score} closed
r = admin.role                                   #@ ok role
#@value r: exactly={"admin"}
act = admin.active                               #@ ok active
#@value act: exactly={True}
plain = User(id=2, name="bo")
sc = plain.score
#@value sc: exactly={0.0}

if __name__ == "__main__":
    make({"id": 1, "name": "a"})
    make({"id": 2, "name": "b", "role": "admin", "extra": 1})
    try:
        make({"id": 3})
    except ValidationError:
        pass
    for u in (admin, plain):
        try:
            email_of(u)
        except AttributeError:
            pass

#@schema param make.payload: always={id,name} only={id,name}

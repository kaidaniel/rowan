# @field_validator (after): the value is normalised; ValidationInfo.data holds the fields validated before this one
from pydantic import BaseModel, ValidationError, ValidationInfo, field_validator


class Member(BaseModel):
    name: str
    email: str
    age: int = 0

    @field_validator("name")
    @classmethod
    def tidy(cls, v):
        return v.strip().title()

    @field_validator("email")
    @classmethod
    def lower(cls, v):
        if "@" not in v:
            raise ValueError("bad email")
        return v.lower()

    @field_validator("age")
    @classmethod
    def plausible(cls, v, info: ValidationInfo):
        data = info.data
        #@row data: present={name,email} absent={age} closed
        if v < 0:
            raise ValueError("negative")
        return v


def enroll(d):
    m = Member(**d)                              #@ possibly email
    #@row m: present={name,email,age} definite={name,email,age} closed
    return m.email


m = Member(name="  ann lee ", email="A@X.ORG")
nm = m.name
#@value nm: exactly={"Ann Lee"}
em = m.email
#@value em: exactly={"a@x.org"}

if __name__ == "__main__":
    enroll({"name": "b", "email": "B@y", "age": 3})
    for bad in ({"name": "c"}, {"name": "d", "email": "nope", "age": 1}, {"name": "e", "email": "e@z", "age": -1}):
        try:
            enroll(bad)
        except ValidationError:
            pass

#@schema param enroll.d: always={name,email} only={name,email}

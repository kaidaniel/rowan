# assigning to a model: fields are assignable (no validation by default), a non-field name is a ValueError, extra="allow" accepts it, validate_assignment validates
from pydantic import BaseModel, ConfigDict, ValidationError


class Cfg(BaseModel):
    name: str
    level: int = 0


class Open(BaseModel):
    model_config = ConfigDict(extra="allow")
    name: str


class Checked(BaseModel):
    model_config = ConfigDict(validate_assignment=True)
    level: int = 0


def set_colour(c):
    c.colour = "red"                             #@ missing colour


def set_nick(c, value):
    setattr(c, "nick", value)                    #@ missing nick


c = Cfg(name="x")
c.level = 3
lv = c.level
#@value lv: exactly={3}
c.level = "not validated"
lv2 = c.level
#@value lv2: exactly={"not validated"}
#@row c: present={name,level} definite={name,level} closed
o = Open(name="o")
o.colour = "blue"                                #@ ok colour
oc = o.colour                                    #@ ok colour
#@value oc: exactly={"blue"}
od = o.model_dump()
#@row od: present={name,colour} definite={name,colour} closed
k = Checked()
k.level = "5"
kl = k.level
#@value kl: exactly={5}

if __name__ == "__main__":
    for m in (c, Cfg(name="y", level=1)):
        try:
            set_colour(m)
        except ValueError:
            pass
        try:
            set_nick(m, "n")
        except ValueError:
            pass
    try:
        k.level = "high"
    except ValidationError:
        pass

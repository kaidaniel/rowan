# extra="forbid" rejects unknown keys (a ValidationError, not a missing key); extra="ignore" (the default) drops them
from pydantic import BaseModel, ConfigDict, ValidationError


class Strict(BaseModel):
    model_config = ConfigDict(extra="forbid")
    a: int
    b: int = 0


class Lenient(BaseModel):
    model_config = ConfigDict(extra="ignore")
    a: int


def strict_load(d):
    s = Strict.model_validate(d)                 #@ possibly a
    #@row s: present={a,b} definite={a,b} closed
    return s.model_dump()


def lenient_load(d):
    l = Lenient.model_validate(d)
    #@row l: present={a} definite={a} closed
    out = l.model_dump()
    #@row out: present={a} definite={a} closed
    return out


def z_of(l):
    return l.z                                   #@ missing z


def rejected(d):
    try:
        Strict.model_validate(d)
        return False
    except ValidationError as e:
        kinds = [err["type"] for err in e.errors()]
        return "extra_forbidden" in kinds


ok = rejected({"a": 1, "zz": 2})
#@value ok: exactly={True}
fine = rejected({"a": 1})
#@value fine: exactly={False}

if __name__ == "__main__":
    strict_load({"a": 1})
    strict_load({"a": 1, "b": 2})
    for bad in ({"a": 1, "c": 3}, {"b": 2}):
        try:
            strict_load(bad)
        except ValidationError:
            pass
    for d in ({"a": 1}, {"a": 2, "z": 9}):
        lenient_load(d)
        try:
            z_of(Lenient(**d))
        except AttributeError:
            pass

#@schema param strict_load.d: always={a} only={a}
#@schema param lenient_load.d: always={a} only={a}

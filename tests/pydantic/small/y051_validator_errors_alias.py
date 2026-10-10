"""Hypothesis: a field validator that raises for some inputs turns into a caught ValidationError
whose errors() list the failing fields in definition order, with `loc` by alias (`userId`) and
`type` (`missing`, `int_parsing`, `value_error`); populate_by_name accepts the field name as
well as the alias; a validator's return value (lowercased) is the stored value; dumps by_alias
use the alias as the key. The caller's result row depends on which branch ran; for a record
from input both branches are possible (schema was only={}: the caught ValidationError is still raised)."""
import json
import os

from pydantic import BaseModel, ConfigDict, Field, ValidationError, field_validator


class User(BaseModel):
    model_config = ConfigDict(populate_by_name=True)
    user_id: int = Field(alias="userId")
    email: str
    age: int = 0

    @field_validator("email")
    @classmethod
    def check_email(cls, v):
        if "@" not in v:
            raise ValueError("no at sign")
        return v.lower()


def register(data):
    try:
        u = User.model_validate(data)
    except ValidationError as e:
        errs = e.errors()
        return {"ok": False, "where": [str(er["loc"][0]) for er in errs], "types": [er["type"] for er in errs]}
    return {"ok": True, "user": u.model_dump(by_alias=True)}


def user_email(res):
    return res["user"]["email"]                  #@ possibly user


if __name__ == "__main__":
    r1 = register({"userId": 1, "email": "A@B.c"})
    #@row r1: present={ok,user} definite={ok,user} absent={where,types} closed
    u1 = r1["user"]                              #@ ok user
    #@row u1: present={userId,email,age} definite={userId,email,age} absent={user_id} closed
    em = u1["email"]                             #@value em: exactly={"a@b.c"}
    r2 = register({"user_id": 2, "email": "x@y"})
    uid = r2["user"]["userId"]                   #@value uid: exactly={2}
    r3 = register({"userId": 3, "email": "bad"})
    #@row r3: present={ok,where,types} definite={ok,where,types} absent={user} closed
    w3 = r3["where"][0]                          #@value w3: exactly={"email"}
    t3 = r3["types"][0]                          #@value t3: exactly={"value_error"}
    r4 = register({"email": "a@b"})
    w4 = r4["where"][0]                          #@value w4: exactly={"userId"}
    r5 = register({"userId": "x", "email": "bad"})
    t5 = tuple(r5["types"])                      #@value t5: exactly={("int_parsing","value_error")}
    for res in (r1, r3):
        try:
            user_email(res)
        except KeyError:
            pass
    plain = User(user_id=5, email="Q@R").model_dump()
    #@row plain: present={user_id,email,age} definite={user_id,email,age} absent={userId} closed
    r6 = register(json.loads(os.environ.get("USER_JSON", '{"userId": 9, "email": "z@z"}')))
    #@schema json.loads@L63: always={email} sometimes={userId,user_id} only={email,userId,user_id}
    _ = 0                                        #@row r6: present={ok,user,where,types} definite={ok} closed

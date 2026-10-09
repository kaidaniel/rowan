# ValidationError.errors(): one dict per problem (type, loc, msg, input, url; ctx for validator errors); turning them into a field -> message map
from pydantic import BaseModel, ValidationError, field_validator


class Signup(BaseModel):
    user: str
    age: int

    @field_validator("age")
    @classmethod
    def adult(cls, v):
        if v < 18:
            raise ValueError("too young")
        return v


def problems(d):
    try:
        Signup.model_validate(d)
        return {}
    except ValidationError as e:
        out = {}
        for err in e.errors():
            #@row err: present={type,loc,msg,input,url,ctx} definite={type,loc,msg,input,url} closed
            out[err["loc"][0]] = err["msg"]      #@ ok loc ; ok msg
        for err in e.errors(include_url=False):
            #@row err: present={type,loc,msg,input,ctx} definite={type,loc,msg,input} absent={url} closed
            kind = err["type"]                   #@ ok type
        return out


def first_type(d):
    try:
        Signup(**d)
    except ValidationError as e:
        return e.errors()[0]["type"]
    return None


t1 = first_type({"user": "a"})
#@value t1: exactly={"missing"}
t2 = first_type({"user": "a", "age": 3})
#@value t2: exactly={"value_error"}
t3 = first_type({"user": "a", "age": "x"})
#@value t3: exactly={"int_parsing"}
t4 = first_type({"user": "a", "age": 30})
#@value t4: exactly={None}
p = problems({"age": 1})
#@row p: present={user,age} definite={user,age} closed
q = problems({"user": "u", "age": 40})
#@row q: present={} closed

if __name__ == "__main__":
    problems({})
    problems({"user": 5, "age": "old"})

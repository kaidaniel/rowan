"""Weakness: unmodelled libraries. A pydantic model has its declared fields; `model_dump()`
has them as keys.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""
from pydantic import BaseModel


class User(BaseModel):
    name: str
    age: int = 0


u = User(name="a")
u.name                                       #@ ok name
u.age                                        #@ sound age
d = u.model_dump()
d["name"]                                    #@ ok name
d["age"]                                     #@ sound age

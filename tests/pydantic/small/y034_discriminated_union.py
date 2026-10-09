# discriminated unions: Literal tags + Field(discriminator="kind"); the tag picks the member class (and its required fields)
from typing import Annotated, Literal, Union

from pydantic import BaseModel, Field, TypeAdapter, ValidationError


class Cat(BaseModel):
    kind: Literal["cat"]
    lives: int = 9


class Dog(BaseModel):
    kind: Literal["dog"]
    breed: str
    good: bool = True


Pet = Annotated[Union[Cat, Dog], Field(discriminator="kind")]


class Owner(BaseModel):
    name: str
    pet: Pet


def adopt(d):
    o = Owner.model_validate(d)                  #@ possibly breed
    pet = o.pet
    #@row pet: present={kind,lives,breed,good} definite={kind} closed
    if isinstance(pet, Dog):
        b = pet.breed                            #@ ok breed
        #@row pet: present={kind,breed,good} definite={kind,breed,good} closed
    return pet


def breed_of(pet):
    return pet.breed                             #@ possibly breed


pets = TypeAdapter(list[Pet]).validate_python([{"kind": "cat"}, {"kind": "dog", "breed": "lab"}])
first = pets[0]
#@row first: present={kind,lives} definite={kind,lives} absent={breed} closed
lv = first.lives
#@value lv: exactly={9}
second = pets[1]
#@row second: present={kind,breed,good} definite={kind,breed,good} closed

if __name__ == "__main__":
    for d in ({"name": "a", "pet": {"kind": "cat"}}, {"name": "b", "pet": {"kind": "dog", "breed": "pug"}}, {"name": "c", "pet": {"kind": "dog"}}):
        try:
            breed_of(adopt(d))
        except (AttributeError, ValidationError):
            pass

#@schema param adopt.d: always={name,pet} only={name,pet}
#@schema param adopt.d['pet']: always={kind} sometimes={breed} only={kind,breed}

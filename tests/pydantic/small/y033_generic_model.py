# generic models (BaseModel, Generic[T]): Page[User] validates its items as User; one envelope parametrised per caller
from typing import Generic, TypeVar

from pydantic import BaseModel, ValidationError

T = TypeVar("T")


class User(BaseModel):
    id: int
    name: str


class Team(BaseModel):
    title: str


class Page(BaseModel, Generic[T]):
    items: list[T]
    total: int
    next: str | None = None


class Envelope(BaseModel, Generic[T]):
    data: T
    error: str | None = None


def users_page(payload):
    page = Page[User].model_validate(payload)    #@ possibly name
    #@row page: present={items,total,next} definite={items,total,next} closed
    for u in page.items:
        #@row u: present={id,name} definite={id,name} closed
        nm = u.name                              #@ ok name
    return page


def wrap(cls, payload):
    env = Envelope[cls].model_validate({"data": payload})
    #@row env: present={data,error} definite={data,error} closed
    inner = env.data
    #@row inner: present={id,name,title} closed
    return env.model_dump()


nums = Page[int](items=[1, "2"], total=2)
second = nums.items[1]
#@value second: exactly={2}
e = wrap(Team, {"title": "core"})
#@row e: present={data,error} definite={data,error} closed
ed = e["data"]
#@row ed: present={title} definite={title} closed

if __name__ == "__main__":
    users_page({"items": [], "total": 0})
    users_page({"items": [{"id": 1, "name": "a"}], "total": 1, "next": "/p2"})
    try:
        users_page({"items": [{"id": 2}], "total": 1})
    except ValidationError:
        pass
    wrap(User, {"id": 3, "name": "c"})

#@schema param users_page.payload: always={items,total} only={items,total}
#@schema param users_page.payload['items']['*']: always={id,name} only={id,name}

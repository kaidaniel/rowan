from dataclasses import dataclass, asdict
from typing import TypedDict


class User(TypedDict):
    id: int
    name: str


@dataclass
class Point:
    x: float
    y: float
    label: str = ""


def greet(u: User):
    return u["name"] + u["nam"]                 #@ ok name ; missing nam


pt = Point(1.0, 2.0)
pt.x                                            #@ ok x
pt.z                                            #@ missing z
d = asdict(pt)
d["label"]                                      #@ ok label
d["labl"]                                       #@ missing labl
u = User(id=1, name="k")
u["id"]                                         #@ ok id
u["email"]                                      #@ missing email

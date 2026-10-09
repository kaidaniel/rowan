# ConfigDict(extra="allow"): extra input keys are kept (model_extra, attribute access, dumped); the join over the callers decides which extras there may be
from pydantic import BaseModel, ConfigDict


class Flexible(BaseModel):
    model_config = ConfigDict(extra="allow")
    id: int
    kind: str = "plain"


def absorb(d):
    f = Flexible(**d)
    #@row f: present={id,kind,color,size} definite={id,kind} closed
    ex = f.model_extra
    #@row ex: present={color,size} absent={id,kind} closed
    dumped = f.model_dump()
    #@row dumped: present={id,kind,color,size} definite={id,kind} closed
    return dumped


def colour(f):
    return f.color                               #@ possibly color


g = Flexible(id=1, color="red")
c = g.color                                      #@ ok color
#@value c: exactly={"red"}
gx = g.model_extra
#@row gx: present={color} definite={color} closed
gd = g.model_dump()
#@row gd: present={id,kind,color} definite={id,kind,color} closed
g.flag = True
gd2 = g.model_dump()
#@row gd2: present={id,kind,color,flag} definite={id,kind,color,flag} closed

if __name__ == "__main__":
    for d in ({"id": 1}, {"id": 2, "color": "blue", "size": 3}, {"id": 3, "kind": "k"}):
        f = Flexible(**d)
        absorb(d)
        try:
            colour(f)
        except AttributeError:
            pass

#@schema param absorb.d: always={id} only={id}

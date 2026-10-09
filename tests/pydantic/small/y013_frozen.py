# frozen=True: assignment fails (ValidationError), instances hash by value; model_copy(update=) makes a changed copy
from pydantic import BaseModel, ConfigDict, ValidationError


class Point(BaseModel):
    model_config = ConfigDict(frozen=True)
    x: int
    y: int = 0


def move(p, dx):
    try:
        p.x = p.x + dx
    except ValidationError:
        pass
    q = p.model_copy(update={"x": p.x + dx})
    #@row q: present={x,y} definite={x,y} closed
    return q


p = Point(x=1)
q = move(p, 2)
px = p.x
#@value px: exactly={1}
qx = q.x
#@value qx: exactly={3}
qy = q.y
#@value qy: exactly={0}
n = len({p, Point(x=1), Point(x=1, y=0)})
#@value n: exactly={1}
r = move(Point(x=5, y=6), -5)
ry = r.y
#@value ry: exactly={6}
d = r.model_dump()
#@row d: present={x,y} definite={x,y} closed

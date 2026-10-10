# nested models of one class in one validation (two fields, a list, a dict of them) are distinct instances
from typing import Dict, List

from pydantic import BaseModel


class A(BaseModel):
    city: str


class P(BaseModel):
    home: A
    work: A
    extra: Dict[str, A] = {}
    hist: List[A] = []


def cities(p):
    return [p.home.city, p.work.city] + [a.city for a in p.hist]


p = P(home={"city": "oslo"}, work={"city": "rome"}, extra={"x": {"city": "x1"}, "y": {"city": "y1"}},
      hist=[{"city": "h1"}, {"city": "h2"}])
h = p.home.city                                      #@value h: exactly={oslo}
w = p.work.city                                      #@value w: exactly={rome}
ex = p.extra["x"].city                               #@value ex: exactly={x1}
ey = p.extra["y"].city                               #@value ey: exactly={y1}
h1 = p.hist[1].city                                  #@value h1: exactly={h2}
cs = cities(p)
c0 = cs[0]                                           #@value c0: exactly={oslo}
c3 = cs[3]                                           #@value c3: exactly={h2}
d = p.model_dump()
dw = d["work"]
#@row dw: present={city} definite={city} closed
dwc = dw["city"]                                     #@value dwc: exactly={rome}

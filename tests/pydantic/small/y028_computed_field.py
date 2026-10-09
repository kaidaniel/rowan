# @computed_field properties are dumped (model_dump / model_dump_json) but are not instance attributes or inputs; a plain @property is neither
import json

from pydantic import BaseModel, computed_field


class Rect(BaseModel):
    w: float
    h: float

    @computed_field
    @property
    def area(self) -> float:
        return self.w * self.h

    @property
    def ratio(self) -> float:
        return self.w / self.h


def report(d):
    r = Rect(**d)
    #@row r: present={w,h} definite={w,h} absent={area,ratio} closed
    out = r.model_dump()
    #@row out: present={w,h,area} definite={w,h,area} absent={ratio} closed
    a = out["area"]                              #@ ok area
    slim = r.model_dump(exclude={"area"})
    #@row slim: present={w,h} definite={w,h} absent={area} closed
    return a


r = Rect(w=2, h=3)
ar = r.area                                      #@ ok area
#@value ar: exactly={6.0}
j = json.loads(r.model_dump_json())
#@row j: present={w,h,area} definite={w,h,area} closed
ignored = Rect(w=1, h=1, area=99).area
#@value ignored: exactly={1.0}

if __name__ == "__main__":
    report({"w": 1, "h": 2})
    report({"w": 3, "h": 4, "area": 0})

#@schema param report.d: always={w,h} only={w,h}

# model_json_schema(): properties keyed by field name (alias when given), "required" only when a field is required, nested models under $defs
from pydantic import BaseModel, Field, computed_field


class Dims(BaseModel):
    w: int = 1
    h: int = 1


class Box(BaseModel):
    label: str = Field(alias="Label")
    dims: Dims
    weight: float | None = None

    @computed_field
    @property
    def area(self) -> int:
        return self.dims.w * self.dims.h


s = Box.model_json_schema()
#@row s: present={properties,required,title,type,"$defs"} definite={properties,required,title,type,"$defs"} closed
props = s["properties"]                          #@ ok properties
#@row props: present={Label,dims,weight} definite={Label,dims,weight} absent={label,area} closed
req = s["required"]
n = len(req)
#@value n: exactly={2}
title = s["title"]
#@value title: exactly={"Box"}
d = Dims.model_json_schema()
#@row d: present={properties,title,type} definite={properties,title,type} absent={required} closed
dp = d["properties"]
#@row dp: present={w,h} definite={w,h} closed
ser = Box.model_json_schema(mode="serialization")["properties"]
#@row ser: present={Label,dims,weight,area} definite={Label,dims,weight,area} closed
plain = Box.model_json_schema(by_alias=False)["properties"]
#@row plain: present={label,dims,weight} definite={label,dims,weight} absent={Label} closed


def required_of(cls):
    return cls.model_json_schema().get("required", [])


r1 = len(required_of(Dims))
#@value r1: exactly={0}

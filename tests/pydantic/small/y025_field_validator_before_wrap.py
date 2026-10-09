# before / wrap field validators: a before validator sees the raw input value (a KeyError in it escapes the validation as a KeyError)
from pydantic import BaseModel, ValidationError, field_validator


class Upload(BaseModel):
    tags: list[str]
    size: int = 0
    meta: dict

    @field_validator("tags", mode="before")
    @classmethod
    def split(cls, v):
        if isinstance(v, str):
            return [t.strip() for t in v.split(",")]
        return v

    @field_validator("size", mode="wrap")
    @classmethod
    def clamp(cls, v, handler):
        try:
            return handler(v)
        except ValidationError:
            return 0

    @field_validator("meta", mode="before")
    @classmethod
    def need_kind(cls, v):
        kind = v["kind"]                         #@ possibly kind
        return {"kind": kind, "source": v.get("source", "web")}


def accept(d):
    u = Upload.model_validate(d)
    #@row u: present={tags,size,meta} definite={tags,size,meta} closed
    m = u.meta
    #@row m: present={kind,source} definite={kind,source} closed
    return m["source"]                           #@ ok source


u = Upload(tags="a, b", size="big", meta={"kind": "img", "x": 1})
n = len(u.tags)
#@value n: exactly={2}
sz = u.size
#@value sz: exactly={0}
um = u.meta
#@row um: present={kind,source} definite={kind,source} absent={x} closed

if __name__ == "__main__":
    accept({"tags": ["x"], "meta": {"kind": "a"}})
    accept({"tags": "p,q", "size": 3, "meta": {"kind": "b", "source": "api"}})
    try:
        accept({"tags": [], "meta": {"source": "cli"}})
    except KeyError:
        pass

#@schema param accept.d: always={tags,meta} only={tags,meta}
#@schema param accept.d['meta']: always={kind} only={kind}

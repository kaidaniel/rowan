# Field(default_factory=…): fresh containers per instance; the factory's value shapes the attribute
from pydantic import BaseModel, Field


def default_limits():
    return {"cpu": 1, "mem": 512}


class Job(BaseModel):
    name: str
    tags: list[str] = Field(default_factory=list)
    limits: dict[str, int] = Field(default_factory=default_limits)
    meta: dict = Field(default_factory=dict)


def submit(name, extra_limits):
    j = Job(name=name)
    #@row j: present={name,tags,limits,meta} definite={name,tags,limits,meta} closed
    lim = j.limits
    #@row lim: present={cpu,mem} definite={cpu,mem} closed
    lim.update(extra_limits)
    cpu = lim["cpu"]                             #@ ok cpu
    j.meta["owner"] = name
    m = j.meta
    #@row m: present={owner} definite={owner} closed
    return j


a = Job(name="a")
b = Job(name="b")
a.tags.append("x")
n = len(b.tags)
#@value n: exactly={0}
lim_b = b.limits
#@row lim_b: present={cpu,mem} definite={cpu,mem} closed
gpu = lim_b.get("gpu")
#@value gpu: exactly={None}

if __name__ == "__main__":
    submit("one", {})
    submit("two", {"gpu": 1})
    submit("three", {"cpu": 4, "disk": 10})

# model_copy(update=…): no validation; an update key that is not a field still lands in the copy's attributes (but is not dumped); deep copies
from pydantic import BaseModel


class Draft(BaseModel):
    title: str
    body: str = ""
    tags: list[str] = []


def bump(d, patch):
    nd = d.model_copy(update=patch)
    #@row nd: present={title,body,tags,reviewer} definite={title,body,tags} closed
    out = nd.model_dump()
    #@row out: present={title,body,tags} definite={title,body,tags} absent={reviewer} closed
    return nd


base = Draft(title="t", tags=["a"])
c1 = base.model_copy(update={"body": "b"})
b1 = c1.body
#@value b1: exactly={"b"}
t1 = base.body
#@value t1: exactly={""}
c2 = base.model_copy(update={"title": 42})
t2 = c2.title
#@value t2: exactly={42}
c3 = base.model_copy(update={"reviewer": "r"})
#@row c3: present={title,body,tags,reviewer} definite={title,body,tags,reviewer} closed
rv = c3.reviewer                                 #@ ok reviewer
#@value rv: exactly={"r"}
d3 = c3.model_dump()
#@row d3: present={title,body,tags} definite={title,body,tags} absent={reviewer} closed
shallow = base.model_copy()
shallow.tags.append("s")
deep = base.model_copy(deep=True)
deep.tags.append("d")
n = len(base.tags)
#@value n: exactly={2}

if __name__ == "__main__":
    bump(base, {})
    bump(base, {"body": "x"})
    bump(base, {"reviewer": "q", "title": "u"})

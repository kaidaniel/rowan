# RootModel: the instance's one attribute is `root`; RootModel[dict] / RootModel[list[Model]] dump to the root value
from pydantic import BaseModel, RootModel, ValidationError


class Item(BaseModel):
    name: str
    qty: int = 1


class Scores(RootModel[dict[str, int]]):
    pass


class Batch(RootModel[list[Item]]):
    pass


def load_batch(rows):
    b = Batch.model_validate(rows)               #@ possibly name
    #@row b: present={root} definite={root} closed
    for it in b.root:
        #@row it: present={name,qty} definite={name,qty} closed
        n = it.name                              #@ ok name
    out = b.model_dump()
    for d in out:
        #@row d: present={name,qty} definite={name,qty} closed
        q = d["qty"]                             #@ ok qty
    return out


s = Scores({"ann": "3", "bo": 4})
#@row s: present={root} definite={root} closed
sd = s.model_dump()
#@row sd: present={ann,bo} definite={ann,bo} closed
a = sd["ann"]
#@value a: exactly={3}
r = s.root
#@row r: present={ann,bo} definite={ann,bo} closed
n = len(Batch([{"name": "x"}, {"name": "y"}]).root)
#@value n: exactly={2}

if __name__ == "__main__":
    load_batch([{"name": "a"}])
    load_batch([{"name": "b", "qty": 2}, {"name": "c", "z": 1}])
    try:
        load_batch([{"qty": 1}])
    except ValidationError:
        pass

#@schema param load_batch.rows['*']: always={name} only={name}

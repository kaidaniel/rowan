# ClassVar and private attributes (_x, PrivateAttr) are not fields: not in the instance's attributes, not dumped, not input
from typing import ClassVar

from pydantic import BaseModel, PrivateAttr


class Counter(BaseModel):
    kind: ClassVar[str] = "counter"
    limit: ClassVar[int] = 10
    _hits: int = PrivateAttr(default=0)
    _cache: dict = PrivateAttr(default_factory=dict)
    name: str
    start: int = 0

    def hit(self):
        self._hits += 1
        self._cache["last"] = self.name
        return self._hits


def use(d):
    c = Counter(**d)
    #@row c: present={name,start} definite={name,start} absent={kind,limit,_hits,_cache} closed
    k = c.kind                                   #@ ok kind
    #@value k: exactly={"counter"}
    n = c.hit()
    #@value n: exactly={1}
    cache = c._cache                             #@ ok _cache
    #@row cache: present={last} definite={last} closed
    out = c.model_dump()
    #@row out: present={name,start} definite={name,start} absent={kind,_hits} closed
    return out


fields = Counter.model_fields
#@row fields: present={name,start} definite={name,start} closed
priv = Counter(name="p").__pydantic_private__
#@row priv: present={_hits,_cache} definite={_hits,_cache} closed

if __name__ == "__main__":
    use({"name": "a"})
    use({"name": "b", "start": 3, "kind": "other"})
    use({"name": "c", "_hits": 5})

#@schema param use.d: always={name} only={name}

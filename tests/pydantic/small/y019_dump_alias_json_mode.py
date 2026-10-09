# model_dump(by_alias=True) and mode="json": the same keys (aliased), values made JSON-compatible
from datetime import datetime
from decimal import Decimal

from pydantic import BaseModel, Field


class Payment(BaseModel):
    when: datetime
    amount: Decimal
    tags: set[str] = set()
    kind: str = Field(alias="type")


def export(p):
    d = p.model_dump(mode="json", by_alias=True)
    #@row d: present={when,amount,tags,type} definite={when,amount,tags,type} absent={kind} closed
    return d["type"]                             #@ ok type


p = Payment(when=datetime(2024, 1, 2, 3, 4, 5), amount=Decimal("1.50"), tags={"a"}, type="card")
j = p.model_dump(mode="json")
#@row j: present={when,amount,tags,kind} definite={when,amount,tags,kind} absent={type} closed
w = j["when"]
#@value w: exactly={"2024-01-02T03:04:05"}
amt = j["amount"]
#@value amt: exactly={"1.50"}
is_list = isinstance(j["tags"], list)
#@value is_list: exactly={True}
py = p.model_dump()
is_dec = isinstance(py["amount"], Decimal)
#@value is_dec: exactly={True}

if __name__ == "__main__":
    export(p)
    export(Payment(when="2024-05-06T00:00:00", amount="3", type="cash"))

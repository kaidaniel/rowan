# @model_validator(mode="before") gets the raw input dict: its reads are requirements of the input (a KeyError there escapes)
from pydantic import BaseModel, ValidationError, model_validator


class Order(BaseModel):
    sku: str
    qty: int
    unit: str = "pcs"

    @model_validator(mode="before")
    @classmethod
    def upgrade(cls, data):
        version = data["version"]                #@ possibly version
        if version == 1:
            return {"sku": data["code"], "qty": data["n"], "unit": "pcs"}   #@ ok code ; ok n
        out = dict(data)
        out.pop("version")
        #@row out: present={sku,qty,unit} definite={sku} absent={version} closed
        return out


def place(d):
    o = Order.model_validate(d)                  #@ possibly version
    #@row o: present={sku,qty,unit} definite={sku,qty,unit} absent={version,code,n} closed
    return o


v1 = Order.model_validate({"version": 1, "code": "A", "n": 2})
s1 = v1.sku
#@value s1: exactly={"A"}
u1 = v1.unit
#@value u1: exactly={"pcs"}

if __name__ == "__main__":
    place({"version": 1, "code": "B", "n": 1})
    place({"version": 2, "sku": "C", "qty": 3, "unit": "kg"})
    try:
        place({"sku": "D", "qty": 1})
    except KeyError:
        pass
    try:
        place({"version": 2, "sku": "E"})
    except ValidationError:
        pass

# version 1 inputs need code / n, the others sku / qty
#@schema param place.d: always={version} sometimes={code,n,sku,qty} only={version}

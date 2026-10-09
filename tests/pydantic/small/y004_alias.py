# Field(alias=…): input keys are the aliases, attributes are the field names, model_dump(by_alias=True) gives the aliases back
from pydantic import BaseModel, Field, ValidationError


class Item(BaseModel):
    item_id: int = Field(alias="itemId")
    display_name: str = Field(alias="displayName")
    unit_price: float = Field(0.0, alias="unitPrice")


def parse_item(raw):
    it = Item.model_validate(raw)                #@ possibly displayName
    #@row it: present={item_id,display_name,unit_price} definite={item_id,display_name,unit_price} absent={itemId,displayName,unitPrice} closed
    out = it.model_dump()
    #@row out: present={item_id,display_name,unit_price} definite={item_id,display_name,unit_price} closed
    wire = it.model_dump(by_alias=True)
    #@row wire: present={itemId,displayName,unitPrice} definite={itemId,displayName,unitPrice} absent={item_id} closed
    return wire["itemId"]                        #@ ok itemId


def by_field_name():
    # without populate_by_name the field names are not accepted as input keys
    return Item(item_id=1, display_name="x")     #@ missing itemId ; missing displayName


it = Item(itemId=5, displayName="five")
#@row it: present={item_id,display_name,unit_price} definite={item_id,display_name,unit_price} closed
dn = it.display_name                             #@ ok display_name
#@value dn: exactly={"five"}


def camel_attr(m):
    return m.displayName                         #@ missing displayName

if __name__ == "__main__":
    parse_item({"itemId": 1, "displayName": "a"})
    parse_item({"itemId": 2, "displayName": "b", "unitPrice": 2.5, "item_id": 9})
    try:
        parse_item({"itemId": 3, "display_name": "c"})
    except ValidationError:
        pass
    try:
        by_field_name()
    except ValidationError:
        pass
    try:
        camel_attr(it)
    except AttributeError:
        pass

#@schema param parse_item.raw: always={itemId,displayName} only={itemId,displayName}

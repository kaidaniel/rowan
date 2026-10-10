"""Request / response models of the order service."""
from typing import List, Optional

from pydantic import BaseModel, ConfigDict, Field, computed_field, model_validator


class Address(BaseModel):
    model_config = ConfigDict(populate_by_name=True)

    street: str
    city: str
    zip_code: str = Field(alias="zip")
    country: str = "NO"


class LineItem(BaseModel):
    model_config = ConfigDict(populate_by_name=True)

    sku: str
    qty: int = 1
    unit_price: str = Field(alias="price")

    @computed_field
    @property
    def line_key(self) -> str:
        return f"{self.sku}x{self.qty}"


class OrderIn(BaseModel):
    model_config = ConfigDict(populate_by_name=True)

    customer: str = Field(alias="customerId")
    items: List[LineItem]
    shipping: Optional[Address] = None
    coupon: Optional[str] = None
    note: Optional[str] = None

    @model_validator(mode="before")
    @classmethod
    def legacy_lines(cls, data):
        # old clients send "lines" instead of "items"
        if isinstance(data, dict) and "lines" in data and "items" not in data:
            data = dict(data)
            data["items"] = data.pop("lines")
        return data

    @computed_field
    @property
    def n_items(self) -> int:
        return sum(i.qty for i in self.items)

    def country(self) -> str:
        return self.shipping.country if self.shipping is not None else "NO"


class OrderOut(BaseModel):
    order_id: str
    customer: str
    total: str
    status: str
    coupon: Optional[str] = None
    tracking: Optional[str] = None

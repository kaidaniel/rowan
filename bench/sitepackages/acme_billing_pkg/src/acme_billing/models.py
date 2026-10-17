"""The records the billing rules work on."""

from dataclasses import dataclass, field
from enum import Enum


class Region(Enum):
    EU = "eu"
    UK = "uk"
    US = "us"
    APAC = "apac"

    @classmethod
    def parse(cls, text: str) -> "Region":
        text = text.strip().lower()
        for member in cls:
            if member.value == text or member.name.lower() == text:
                return member
        raise ValueError(f"unknown region {text!r}")


class Tier(Enum):
    STANDARD = "standard"
    SILVER = "silver"
    GOLD = "gold"
    PARTNER = "partner"


@dataclass
class Product:
    sku: str
    name: str
    unit_price: float
    category: str
    taxable: bool = True
    stock: int = 0

    def in_stock(self, quantity: int) -> bool:
        return self.stock >= quantity


@dataclass
class Customer:
    customer_id: str
    name: str
    region: Region
    tier: Tier = Tier.STANDARD
    vat_id: str | None = None
    lifetime_spend: float = 0.0
    tags: list[str] = field(default_factory=list)

    @property
    def is_business(self) -> bool:
        return self.vat_id is not None

    def label(self) -> str:
        return f"{self.name} ({self.customer_id})"


@dataclass
class LineItem:
    sku: str
    description: str
    quantity: int
    unit_price: float
    discount: float = 0.0
    tax: float = 0.0

    @property
    def net(self) -> float:
        return round(self.quantity * self.unit_price - self.discount, 2)

    @property
    def gross(self) -> float:
        return round(self.net + self.tax, 2)


@dataclass
class Invoice:
    number: str
    customer: Customer
    currency: str
    lines: list[LineItem] = field(default_factory=list)
    shipping: float = 0.0
    order_discount: float = 0.0
    issued: str = ""
    due: str = ""
    notes: list[str] = field(default_factory=list)

    @property
    def subtotal(self) -> float:
        return round(sum(line.net for line in self.lines), 2)

    @property
    def tax(self) -> float:
        return round(sum(line.tax for line in self.lines), 2)

    @property
    def total(self) -> float:
        return round(self.subtotal - self.order_discount + self.tax + self.shipping, 2)

    def line_count(self) -> int:
        return len(self.lines)

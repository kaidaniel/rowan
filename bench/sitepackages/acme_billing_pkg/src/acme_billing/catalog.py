"""The product catalogue (a snapshot the nightly sync writes; read-only here)."""

from .errors import PricingError
from .models import Product

CATALOG: dict[str, dict] = {
    "A100": {"name": "Widget, small", "unit_price": 4.5, "category": "widgets", "taxable": True, "stock": 1200},
    "A200": {"name": "Widget, large", "unit_price": 9.0, "category": "widgets", "taxable": True, "stock": 300},
    "B310": {"name": "Gadget", "unit_price": 24.99, "category": "gadgets", "taxable": True, "stock": 85},
    "B320": {"name": "Gadget Pro", "unit_price": 59.0, "category": "gadgets", "taxable": True, "stock": 12},
    "C001": {"name": "Support hour", "unit_price": 120.0, "category": "services", "taxable": False, "stock": 10_000},
    "C002": {"name": "Onboarding", "unit_price": 450.0, "category": "services", "taxable": False, "stock": 10_000},
    "D900": {"name": "Gift card", "unit_price": 50.0, "category": "vouchers", "taxable": False, "stock": 500},
}

DISCONTINUED = {"A050", "B300"}


def lookup_product(sku: str) -> Product:
    """The catalogue entry for `sku` as a Product; PricingError if there is none."""
    sku = sku.strip().upper()
    if sku in DISCONTINUED:
        raise PricingError(f"product {sku} is discontinued", sku=sku)
    entry = CATALOG.get(sku)
    if entry is None:
        raise PricingError(f"unknown product {sku}", sku=sku)
    return Product(
        sku=sku,
        name=entry["name"],
        unit_price=entry["unit_price"],
        category=entry["category"],
        taxable=entry.get("taxable", True),
        stock=entry.get("stock", 0),
    )


def products_in(category: str) -> list[Product]:
    """Every product of a category, cheapest first."""
    found = [lookup_product(sku) for sku, entry in CATALOG.items() if entry["category"] == category]
    found.sort(key=lambda p: p.unit_price)
    return found


def categories() -> list[str]:
    return sorted({entry["category"] for entry in CATALOG.values()})

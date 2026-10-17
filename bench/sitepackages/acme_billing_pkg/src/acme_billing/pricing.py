"""Pricing rules: tier discounts, volume breaks, coupons, tax and shipping."""

from .catalog import lookup_product
from .config import DEFAULTS
from .errors import PricingError
from .models import Customer, LineItem, Region, Tier
from .money import convert, round_money

TIER_DISCOUNT = {Tier.STANDARD: 0.0, Tier.SILVER: 0.03, Tier.GOLD: 0.07, Tier.PARTNER: 0.15}

# (minimum quantity, discount) per category, largest first
VOLUME_BREAKS = {
    "widgets": [(1000, 0.12), (500, 0.08), (100, 0.04)],
    "gadgets": [(50, 0.06), (10, 0.03)],
    "services": [],
    "vouchers": [],
}

TAX_RATES = {
    Region.EU: {"standard": 0.21, "services": 0.21, "vouchers": 0.0},
    Region.UK: {"standard": 0.20, "services": 0.20, "vouchers": 0.0},
    Region.US: {"standard": 0.0725, "services": 0.0, "vouchers": 0.0},
    Region.APAC: {"standard": 0.10, "services": 0.10, "vouchers": 0.0},
}

COUPONS = {
    "WELCOME10": {"kind": "percent", "value": 0.10, "min_subtotal": 0.0},
    "BULK50": {"kind": "fixed", "value": 50.0, "min_subtotal": 500.0},
    "FREESHIP": {"kind": "shipping", "value": 0.0, "min_subtotal": 0.0},
}


def volume_discount(category: str, quantity: int) -> float:
    for minimum, rate in VOLUME_BREAKS.get(category, []):
        if quantity >= minimum:
            return rate
    return 0.0


def tax_rate(region: Region, category: str, business: bool) -> float:
    """The tax rate of a category in a region; EU business customers reverse-charge."""
    if business and region is Region.EU:
        return 0.0
    rates = TAX_RATES[region]
    return rates.get(category, rates["standard"])


def price_line(line: dict, customer: Customer, currency: str = "EUR") -> LineItem:
    """A priced LineItem for an order line {"sku", "qty", "unit_price"?}."""
    product = lookup_product(line["sku"])
    quantity = line["qty"]
    unit = line.get("unit_price", product.unit_price)
    unit = convert(unit, "EUR", currency)
    rate = max(TIER_DISCOUNT[customer.tier], volume_discount(product.category, quantity))
    gross = quantity * unit
    discount = round_money(gross * rate)
    tax = 0.0
    if product.taxable:
        tax = round_money((gross - discount) * tax_rate(customer.region, product.category, customer.is_business))
    return LineItem(
        sku=product.sku,
        description=product.name,
        quantity=quantity,
        unit_price=unit,
        discount=discount,
        tax=tax,
    )


def order_discount(subtotal: float, coupon: str | None) -> tuple[float, bool]:
    """(amount off the subtotal, whether shipping is free) for a coupon code."""
    if coupon is None:
        return 0.0, False
    rule = COUPONS.get(coupon.upper())
    if rule is None:
        raise PricingError(f"unknown coupon {coupon}")
    if subtotal < rule["min_subtotal"]:
        return 0.0, False
    if rule["kind"] == "percent":
        return round_money(subtotal * rule["value"]), False
    if rule["kind"] == "fixed":
        return min(rule["value"], subtotal), False
    return 0.0, True


def shipping_for(subtotal: float, free: bool, settings: dict | None = None) -> float:
    settings = settings or DEFAULTS
    if free or subtotal >= settings["free_shipping_over"]:
        return 0.0
    return settings["shipping_flat"]

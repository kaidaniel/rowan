"""Prices are decimal strings; every amount is rounded half-up to cents."""
from decimal import ROUND_HALF_UP, Decimal

CENT = Decimal("0.01")
TAX_RATES = {"NO": Decimal("0.25"), "SE": Decimal("0.25"), "DE": Decimal("0.19")}
COUPONS = {"TEN": Decimal("0.10"), "HALF": Decimal("0.50")}


def money(x):
    return Decimal(x).quantize(CENT, rounding=ROUND_HALF_UP)


def line_total(item):
    return money(Decimal(item.unit_price) * item.qty)


def register_promo(raw):
    """A promo from the admin side: code and rate required, an optional minimum."""
    code = raw["code"].upper()
    rate = Decimal(raw["rate"])
    if raw.get("min_total") is not None:
        floor = money(raw["min_total"])
    else:
        floor = Decimal("0")
    COUPONS[code] = rate
    return {"code": code, "rate": str(rate), "floor": str(floor)}


def quote(order, country="NO"):
    subtotal = sum((line_total(i) for i in order.items), Decimal("0"))
    discount = Decimal("0")
    if order.coupon is not None:
        rate = COUPONS.get(order.coupon)
        if rate is not None:
            discount = money(subtotal * rate)
    taxable = subtotal - discount
    tax = money(taxable * TAX_RATES.get(country, Decimal("0")))
    q = {"subtotal": str(subtotal), "discount": str(discount), "tax": str(tax), "total": str(taxable + tax)}
    if discount == 0:
        del q["discount"]
    return q

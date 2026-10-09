"""Quotes over carts; relative imports of a sibling module and of the subpackage."""
from . import models
from .util import CURRENCY, money


def quote(cart):
    lines = [{"sku": line["item"].sku, "qty": line["qty"], "cents": line["item"].price * line["qty"]} for line in cart.lines]
    q = {"owner": cart.owner, "lines": lines, "total": cart.total(), "currency": CURRENCY["code"]}
    if cart.owner == "vip":
        q["discount"] = 5
    return q


def summarise(q):
    return {"owner": q["owner"], "n": len(q["lines"]), "total_str": money.fmt(q["total"])}


def empty_cart(owner):
    return models.Cart(owner)

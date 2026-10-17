"""Checks an order passes before it is priced."""

from .catalog import CATALOG, DISCONTINUED
from .config import DEFAULTS
from .errors import ValidationError

REQUIRED_ORDER_KEYS = ("order_id", "customer", "lines")
REQUIRED_LINE_KEYS = ("sku", "qty")
SUPPORTED_CURRENCIES = ("EUR", "GBP", "USD")


def _problem(field: str, message: str) -> dict:
    return {"field": field, "message": message}


def validate_line(line: dict, index: int, allow_backorder: bool) -> list[dict]:
    problems = []
    where = f"lines[{index}]"
    for key in REQUIRED_LINE_KEYS:
        if key not in line:
            problems.append(_problem(f"{where}.{key}", f"{key} is required"))
    if problems:
        return problems
    sku = str(line["sku"]).strip().upper()
    qty = line["qty"]
    if not isinstance(qty, int) or isinstance(qty, bool):
        problems.append(_problem(f"{where}.qty", "quantity must be a whole number"))
    elif qty <= 0:
        problems.append(_problem(f"{where}.qty", "quantity must be positive"))
    if sku in DISCONTINUED:
        problems.append(_problem(f"{where}.sku", f"{sku} is discontinued"))
    elif sku not in CATALOG:
        problems.append(_problem(f"{where}.sku", f"unknown product {sku}"))
    elif isinstance(qty, int) and not allow_backorder and CATALOG[sku]["stock"] < qty:
        problems.append(_problem(f"{where}.qty", f"only {CATALOG[sku]['stock']} of {sku} in stock"))
    if "unit_price" in line and line["unit_price"] < 0:
        problems.append(_problem(f"{where}.unit_price", "price override cannot be negative"))
    return problems


def validate_order(order: dict, settings: dict | None = None) -> list[dict]:
    """Every problem with `order`, as {"field", "message"} dicts (empty: valid)."""
    settings = settings or DEFAULTS
    problems = []
    for key in REQUIRED_ORDER_KEYS:
        if key not in order:
            problems.append(_problem(key, f"{key} is required"))
    if problems:
        return problems
    currency = order.get("currency", settings["currency"])
    if currency not in SUPPORTED_CURRENCIES:
        problems.append(_problem("currency", f"currency {currency} is not supported"))
    lines = order["lines"]
    if not lines:
        problems.append(_problem("lines", "an order needs at least one line"))
    if len(lines) > settings["max_lines"]:
        problems.append(_problem("lines", f"at most {settings['max_lines']} lines"))
    seen = set()
    for i, line in enumerate(lines):
        problems.extend(validate_line(line, i, settings["allow_backorder"]))
        sku = str(line.get("sku", "")).upper()
        if sku and sku in seen:
            problems.append(_problem(f"lines[{i}].sku", f"{sku} appears twice"))
        seen.add(sku)
    coupon = order.get("coupon")
    if coupon is not None and not (isinstance(coupon, str) and coupon.isalnum()):
        problems.append(_problem("coupon", "coupon codes are letters and digits"))
    return problems


def require_valid(order: dict, settings: dict | None = None) -> dict:
    """`order` itself if it is valid; ValidationError with every problem if not."""
    problems = validate_order(order, settings)
    if problems:
        raise ValidationError(problems)
    return order

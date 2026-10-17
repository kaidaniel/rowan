"""Invoices from validated orders."""

from datetime import date, timedelta

from .config import DEFAULTS
from .models import Customer, Invoice
from .pricing import order_discount, price_line, shipping_for
from .validation import require_valid

_counter = {"next": 1}


def invoice_number(prefix: str, issued: date) -> str:
    n = _counter["next"]
    _counter["next"] = n + 1
    return f"{prefix}-{issued.year}{issued.month:02d}-{n:05d}"


def build_invoice(order: dict, customer: Customer, settings: dict | None = None, today: date | None = None) -> Invoice:
    """Validate and price an order into an Invoice for `customer`."""
    settings = settings or DEFAULTS
    require_valid(order, settings)
    issued = today or date(2026, 1, 1)
    currency = order.get("currency", settings["currency"])
    invoice = Invoice(
        number=invoice_number(settings["invoice_prefix"], issued),
        customer=customer,
        currency=currency,
        issued=issued.isoformat(),
        due=(issued + timedelta(days=settings["payment_terms_days"])).isoformat(),
    )
    for line in order["lines"]:
        invoice.lines.append(price_line(line, customer, currency))
    amount, free_shipping = order_discount(invoice.subtotal, order.get("coupon"))
    invoice.order_discount = amount
    invoice.shipping = shipping_for(invoice.subtotal - amount, free_shipping, settings)
    if customer.is_business:
        invoice.notes.append(f"VAT ID {customer.vat_id}")
    if amount:
        invoice.notes.append(f"coupon {order['coupon']}")
    return invoice


def invoice_to_dict(invoice: Invoice) -> dict:
    """The JSON the invoice service stores and the PDF renderer reads."""
    return {
        "number": invoice.number,
        "customer_id": invoice.customer.customer_id,
        "customer": invoice.customer.label(),
        "region": invoice.customer.region.value,
        "currency": invoice.currency,
        "issued": invoice.issued,
        "due": invoice.due,
        "lines": [
            {
                "sku": line.sku,
                "description": line.description,
                "qty": line.quantity,
                "unit_price": line.unit_price,
                "discount": line.discount,
                "tax": line.tax,
                "net": line.net,
            }
            for line in invoice.lines
        ],
        "subtotal": invoice.subtotal,
        "discount": invoice.order_discount,
        "shipping": invoice.shipping,
        "tax": invoice.tax,
        "total": invoice.total,
        "notes": list(invoice.notes),
    }


def late_fee(invoice: dict, days_late: int, settings: dict | None = None) -> float:
    settings = settings or DEFAULTS
    if days_late <= 0:
        return 0.0
    months = (days_late + 29) // 30
    return round(invoice["total"] * settings["late_fee_rate"] * months, 2)

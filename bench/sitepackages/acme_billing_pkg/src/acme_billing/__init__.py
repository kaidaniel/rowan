"""Acme billing: the rules finance signed off on, shared by the order service, the nightly
invoice run and the reporting scripts."""

from .errors import BillingError, PricingError, ValidationError
from .models import Customer, Invoice, LineItem, Product, Region, Tier
from .catalog import CATALOG, lookup_product, products_in
from .customers import customer_from_dict, tier_for
from .validation import validate_order, require_valid
from .pricing import price_line, order_discount, tax_rate
from .invoices import build_invoice, invoice_to_dict, invoice_number
from .reports import report_rows, summarise_by, top_products, render_rows
from .config import DEFAULTS, load_settings

__version__ = "2.4.1"

__all__ = [
    "BillingError", "PricingError", "ValidationError",
    "Customer", "Invoice", "LineItem", "Product", "Region", "Tier",
    "CATALOG", "lookup_product", "products_in",
    "customer_from_dict", "tier_for",
    "validate_order", "require_valid",
    "price_line", "order_discount", "tax_rate",
    "build_invoice", "invoice_to_dict", "invoice_number",
    "report_rows", "summarise_by", "top_products", "render_rows",
    "DEFAULTS", "load_settings",
]

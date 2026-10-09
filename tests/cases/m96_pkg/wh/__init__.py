"""Warehouse package: configuration, models, inventory, receiving, picking, shipping,
reporting and an event bus; the entry module wires them together."""
from . import config, events, inventory, picking, receiving, reports, shipping
from .config import Config, build_config
from .models import Carrier, Location, Lot, Perishable, Product, Shipment, order_record

VERSION = {"major": 0, "minor": 9, "tag": "demo"}

__all__ = ["config", "events", "inventory", "picking", "receiving", "reports", "shipping", "Config", "build_config", "Carrier", "Location", "Lot", "Perishable", "Product", "Shipment", "order_record", "VERSION"]

"""Subpackage re-exporting its module's helpers and holding a constant."""
from .money import cents, fmt

CURRENCY = {"code": "EUR", "minor": 2}

"""Utilities shared by the warehouse package."""
from .paths import deep_get, deep_merge, deep_set, flatten
from .text import slug, money, pad

__all__ = ["deep_get", "deep_merge", "deep_set", "flatten", "slug", "money", "pad"]

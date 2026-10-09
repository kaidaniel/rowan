"""A tiny installed package (lives in a fake site-packages directory next to the case)."""
from .records import Record, load, VERSION

__all__ = ["Record", "load", "VERSION"]

"""Configuration: a literal INI, environment overrides for a fixed set of keys, defaults; the
three layers in a ChainMap (environment first)."""
import configparser
import os
from collections import ChainMap

INI = """
[inventory]
low_stock = 3
currency = EUR

[report]
top = 2
"""

ALT_INI = """
[inventory]
low_stock = 1
"""

DEFAULTS = {"low_stock": "5", "currency": "USD", "top": "3", "title": "Stock"}
ENV_KEYS = ("low_stock", "currency", "title")


def env_overrides():
    out = {}
    for key in ENV_KEYS:
        v = os.environ.get("INV_" + key.upper())
        if v is not None:
            out[key] = v
    return out


def load_config(text=INI):
    cp = configparser.ConfigParser()
    cp.read_string(text)
    file_values = {}
    for section in cp.sections():
        file_values.update(cp[section])
    _ = 0  #@row file_values: present={low_stock,currency,top} definite={low_stock} closed
    return ChainMap(env_overrides(), file_values, DEFAULTS)


def region_of(cfg):
    return cfg["region"]                         #@ missing region

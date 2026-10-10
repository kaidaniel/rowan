"""Settings layer: defaults, a JSON text and environment overrides merged into one dict and
validated by a pydantic model."""
import json
import os
from typing import Literal

from pydantic import BaseModel, Field, ValidationError, field_validator

from dp.registry import REGISTRY

DEFAULTS = {
    "name": "platform",
    "sources": ["csv"],
    "currency": "eur",
    "batch_size": 100,
    "strict": False,
    "threshold": 0.0,
    "export": {"format": "json", "indent": 2, "sort_keys": True},
    "tags": {},
}

# environment variable -> path in the settings dict
ENV_MAP = {
    "DP_CURRENCY": ("currency",),
    "DP_BATCH_SIZE": ("batch_size",),
    "DP_STRICT": ("strict",),
    "DP_THRESHOLD": ("threshold",),
    "DP_EXPORT_FORMAT": ("export", "format"),
    "DP_EXPORT_INDENT": ("export", "indent"),
}


class ExportConfig(BaseModel):
    format: Literal["json", "csv", "text"] = "json"
    indent: int = 2
    sort_keys: bool = True


class PlatformSettings(BaseModel):
    name: str
    sources: list[str]
    currency: str = "EUR"
    batch_size: int = Field(100, gt=0)
    strict: bool = False
    threshold: float = 0.0
    export: ExportConfig = ExportConfig()
    tags: dict[str, str] = {}

    @field_validator("sources")
    @classmethod
    def known_sources(cls, v):
        bad = [s for s in v if not REGISTRY.has("source", s)]
        if bad:
            raise ValueError("unknown sources: " + " ".join(bad))
        return [REGISTRY.canonical("source", s) for s in v]

    @field_validator("currency")
    @classmethod
    def currency_code(cls, v):
        if len(v) != 3:
            raise ValueError("currency must have three letters")
        return v.upper()

    def describe(self):
        return f"{self.name} [{'+'.join(self.sources)}] {self.currency} -> {self.export.format}"


def deep_merge(base, over):
    out = dict(base)
    for key, val in over.items():
        if isinstance(val, dict) and isinstance(out.get(key), dict):
            out[key] = deep_merge(out[key], val)
        else:
            out[key] = val
    return out


def parse_layer(text):
    if not text or not text.strip():
        return {}
    layer = json.loads(text)
    if not isinstance(layer, dict):
        raise ValueError("a config text must hold an object")
    return layer


def env_layer(environ):
    out = {}
    for var, path in ENV_MAP.items():
        if var not in environ:
            continue
        raw = environ[var]
        if len(path) == 1:
            out[path[0]] = raw
        else:
            out.setdefault(path[0], {})[path[1]] = raw
    return out


def merged_config(text, environ=None):
    environ = os.environ if environ is None else environ
    merged = {}
    for layer in (DEFAULTS, parse_layer(text), env_layer(environ)):
        merged = deep_merge(merged, layer)
    return merged


def build_settings(text, environ=None):
    merged = merged_config(text, environ)
    return PlatformSettings.model_validate(merged), merged


def try_settings(text, environ=None):
    try:
        settings, _ = build_settings(text, environ)
    except ValidationError as e:
        return None, [(".".join(str(p) for p in err["loc"]), err["type"]) for err in e.errors()]
    return settings, []

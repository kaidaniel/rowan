"""Settings: defaults finance agreed on, overridable per deployment."""

DEFAULTS = {
    "currency": "EUR",
    "payment_terms_days": 30,
    "late_fee_rate": 0.015,
    "invoice_prefix": "INV",
    "rounding": 2,
    "free_shipping_over": 250.0,
    "shipping_flat": 9.5,
    "max_lines": 200,
    "allow_backorder": False,
}

_INT_KEYS = ("payment_terms_days", "rounding", "max_lines")
_FLOAT_KEYS = ("late_fee_rate", "free_shipping_over", "shipping_flat")
_BOOL_KEYS = ("allow_backorder",)


def _coerce(key: str, value):
    if key in _INT_KEYS:
        return int(value)
    if key in _FLOAT_KEYS:
        return float(value)
    if key in _BOOL_KEYS:
        if isinstance(value, str):
            return value.strip().lower() in ("1", "true", "yes", "on")
        return bool(value)
    return value


def load_settings(overrides: dict | None = None, env: dict | None = None) -> dict:
    """The defaults, then `env`'s ACME_BILLING_* variables, then `overrides`."""
    settings = dict(DEFAULTS)
    for name, value in (env or {}).items():
        if not name.startswith("ACME_BILLING_"):
            continue
        key = name[len("ACME_BILLING_"):].lower()
        if key in settings:
            settings[key] = _coerce(key, value)
    for key, value in (overrides or {}).items():
        if key not in DEFAULTS:
            raise KeyError(f"unknown setting {key!r}")
        settings[key] = _coerce(key, value)
    return settings

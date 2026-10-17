"""Errors the billing rules raise."""


class BillingError(Exception):
    """Base class: anything the billing rules reject."""

    def __init__(self, message: str, *, field: str | None = None, code: str = "billing"):
        super().__init__(message)
        self.message = message
        self.field = field
        self.code = code

    def to_dict(self) -> dict:
        out = {"code": self.code, "message": self.message}
        if self.field is not None:
            out["field"] = self.field
        return out


class ValidationError(BillingError):
    """An order or customer record is malformed."""

    def __init__(self, problems: list[dict]):
        first = problems[0]["message"] if problems else "invalid"
        super().__init__(first, field=problems[0].get("field") if problems else None, code="invalid")
        self.problems = problems

    def to_dict(self) -> dict:
        out = super().to_dict()
        out["problems"] = list(self.problems)
        return out


class PricingError(BillingError):
    """A price cannot be computed (unknown product, bad coupon, currency not supported)."""

    def __init__(self, message: str, *, sku: str | None = None):
        super().__init__(message, field="sku" if sku else None, code="pricing")
        self.sku = sku

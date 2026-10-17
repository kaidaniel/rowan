"""Customer records from the CRM export."""

from .errors import ValidationError
from .models import Customer, Region, Tier

TIER_THRESHOLDS = [(50_000.0, Tier.GOLD), (10_000.0, Tier.SILVER)]


def tier_for(lifetime_spend: float, tags: list[str]) -> Tier:
    """The tier a customer is on: partners by tag, the others by what they spent."""
    if "partner" in tags:
        return Tier.PARTNER
    for threshold, tier in TIER_THRESHOLDS:
        if lifetime_spend >= threshold:
            return tier
    return Tier.STANDARD


def customer_from_dict(record: dict) -> Customer:
    """A Customer from a CRM record: `id`, `name`, `region` required; `vat_id`, `spend`, `tags` optional."""
    problems = []
    for key in ("id", "name", "region"):
        if key not in record or record[key] in (None, ""):
            problems.append({"field": key, "message": f"customer {key} is required"})
    if problems:
        raise ValidationError(problems)
    try:
        region = Region.parse(record["region"])
    except ValueError as exc:
        raise ValidationError([{"field": "region", "message": str(exc)}]) from exc
    tags = [t.strip().lower() for t in record.get("tags", []) if t.strip()]
    spend = float(record.get("spend", 0.0))
    return Customer(
        customer_id=str(record["id"]),
        name=record["name"].strip(),
        region=region,
        tier=tier_for(spend, tags),
        vat_id=record.get("vat_id") or None,
        lifetime_spend=spend,
        tags=tags,
    )


def customer_to_dict(customer: Customer) -> dict:
    return {
        "id": customer.customer_id,
        "name": customer.name,
        "region": customer.region.value,
        "tier": customer.tier.value,
        "vat_id": customer.vat_id,
        "spend": customer.lifetime_spend,
        "tags": list(customer.tags),
    }

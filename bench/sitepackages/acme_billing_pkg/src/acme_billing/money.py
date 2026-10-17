"""Currency conversion and rounding as finance does it."""

from .errors import PricingError

# the treasury's monthly rates, EUR per unit
RATES_TO_EUR = {"EUR": 1.0, "GBP": 1.17, "USD": 0.92}


def round_money(amount: float, digits: int = 2) -> float:
    """Half-up rounding to cents (Python's round() is half-even; finance wants half-up)."""
    scale = 10 ** digits
    value = abs(amount) * scale
    rounded = int(value + 0.5) / scale
    return -rounded if amount < 0 else rounded


def convert(amount: float, source: str, target: str) -> float:
    if source == target:
        return amount
    try:
        eur = amount * RATES_TO_EUR[source]
        return round_money(eur / RATES_TO_EUR[target])
    except KeyError as exc:
        raise PricingError(f"no exchange rate for {exc.args[0]}") from None


def format_money(amount: float, currency: str) -> str:
    symbols = {"EUR": "€", "GBP": "£", "USD": "$"}
    symbol = symbols.get(currency, currency + " ")
    sign = "-" if amount < 0 else ""
    return f"{sign}{symbol}{abs(amount):,.2f}"

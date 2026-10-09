"""Inbound manifests: rows are validated by a chain of rule objects (a class hierarchy
spread over this module), accepted rows become lots, rejected rows are reported."""
from .models import Perishable, Product


class Rule:
    name = "rule"

    def check(self, row, ctx):
        return None


class HasSku(Rule):
    name = "has_sku"

    def check(self, row, ctx):
        if "sku" not in row:
            return {"rule": self.name, "why": "no sku"}
        if row["sku"] not in ctx["inventory"].products:
            return {"rule": self.name, "why": "unknown sku", "sku": row["sku"]}
        return None


class PositiveQty(Rule):
    name = "positive_qty"

    def check(self, row, ctx):
        if row.get("qty", 0) <= 0:
            return {"rule": self.name, "why": "qty"}
        return None


class KnownLocation(Rule):
    name = "known_location"

    def check(self, row, ctx):
        if row.get("loc") not in ctx["inventory"].locations:
            return {"rule": self.name, "why": "location", "loc": row.get("loc")}
        return None


class LotRequired(Rule):
    name = "lot_required"

    def check(self, row, ctx):
        if ctx["cfg"].get("receiving.require_lot") and "lot_ref" not in row:
            return {"rule": self.name, "why": "lot_ref"}
        return None


class ColdChain(Rule):
    name = "cold_chain"

    def check(self, row, ctx):
        product = ctx["inventory"].products.get(row.get("sku"))
        if isinstance(product, Perishable) and not row.get("temp_ok"):
            return {"rule": self.name, "why": "temperature"}
        return None


def default_rules(cfg):
    rules = [HasSku(), PositiveQty(), KnownLocation(), LotRequired()]
    if cfg.feature("cold_chain"):
        rules.append(ColdChain())
    return rules


class Receipt:
    def __init__(self, ref, rows):
        self.ref = ref
        self.rows = list(rows)
        self.accepted = []
        self.rejected = []
        self.lots = []

    def summary(self):
        d = {"ref": self.ref, "rows": len(self.rows), "accepted": len(self.accepted), "rejected": len(self.rejected)}
        if self.rejected:
            d["reasons"] = sorted({r["problem"]["why"] for r in self.rejected})
        return d


class ReceiptProcessor:
    def __init__(self, inventory, cfg, rules=None):
        self.inventory = inventory
        self.cfg = cfg
        self.rules = rules if rules is not None else default_rules(cfg)
        self.processed = {}

    def validate(self, row):
        ctx = {"inventory": self.inventory, "cfg": self.cfg}
        for rule in self.rules:
            problem = rule.check(row, ctx)
            if problem is not None:
                return problem
        return None

    def process(self, ref, rows):
        receipt = Receipt(ref, rows)
        for row in receipt.rows:
            problem = self.validate(row)
            if problem is None:
                lot = self.inventory.receive(row["sku"], row["qty"], row["loc"], row.get("expires", 0), {"ref": ref, "lot_ref": row.get("lot_ref", "")})
                receipt.accepted.append(row)
                receipt.lots.append(lot)
            else:
                receipt.rejected.append({"row": row, "problem": problem})
        self.processed[ref] = receipt
        return receipt

    def stats(self):
        out = {"receipts": len(self.processed), "lots": 0, "rejected": 0}
        for r in self.processed.values():
            out["lots"] += len(r.lots)
            out["rejected"] += len(r.rejected)
        return out


def manifest_rows(spec):
    """Rows from a compact spec: (sku, qty, loc) tuples plus a dict of extras per sku."""
    rows = []
    extras = spec.get("extras", {})
    for sku, qty, loc in spec["items"]:
        row = {"sku": sku, "qty": qty, "loc": loc}
        row.update(extras.get(sku, {}))
        rows.append(row)
    return rows

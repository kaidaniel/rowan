"""Cycle counts and adjustments: a count sheet per location, discrepancies against the
inventory, adjustments applied back, and a maintenance log."""
from .models import Lot


class CountSheet:
    def __init__(self, loc_code):
        self.loc_code = loc_code
        self.counted = {}
        self.closed = False

    def count(self, lot_id, qty):
        self.counted[lot_id] = qty
        return self

    def close(self):
        self.closed = True
        return self


def discrepancies(inventory, sheet):
    out = []
    for lot_id, qty in sheet.counted.items():
        lot = inventory.lots.get(lot_id)
        if lot is None:
            out.append({"lot": lot_id, "why": "unknown", "counted": qty})
        elif lot.qty != qty:
            out.append({"lot": lot_id, "why": "qty", "counted": qty, "system": lot.qty})
    return out


def apply_adjustments(inventory, diffs, log):
    applied = []
    for d in diffs:
        if d["why"] != "qty":
            log.append({"lot": d["lot"], "skipped": True})
            continue
        lot = inventory.lots[d["lot"]]
        delta = d["counted"] - lot.qty
        lot.qty = d["counted"]
        inventory.moves.append({"op": "adjust", "lot": lot.lot_id, "qty": delta})
        entry = {"lot": lot.lot_id, "delta": delta}
        if d["counted"] == 0:
            entry["emptied"] = True
        log.append(entry)
        applied.append(entry)
    return applied


def cycle_count(inventory, loc_code, counts):
    sheet = CountSheet(loc_code)
    for lot_id, qty in counts.items():
        sheet.count(lot_id, qty)
    sheet.close()
    diffs = discrepancies(inventory, sheet)
    log = []
    applied = apply_adjustments(inventory, diffs, log)
    result = {"loc": loc_code, "counted": len(sheet.counted), "diffs": len(diffs), "applied": applied, "log": log}
    if not diffs:
        result["clean"] = True
    return result


def health(inventory):
    empties = [lot_id for lot_id, lot in inventory.lots.items() if lot.qty == 0]
    report = {"lots": len(inventory.lots), "empty": empties, "moves": len(inventory.moves)}
    if empties:
        report["needs_cleanup"] = True
    return report

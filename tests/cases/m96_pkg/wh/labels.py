"""Labels and keys: text built from records, checksum-like codes, and parsing back."""
from .util.text import key_for, pad, parse_key, slug


def location_label(loc):
    return {"text": pad(loc.code, 8) + loc.zone, "key": loc.key(), "zone": loc.zone}


def lot_label(lot):
    base = {"text": lot.lot_id + " " + slug(lot.product.name), "key": key_for("lot", lot.lot_id), "sku": lot.product.sku}
    if lot.expires:
        base["expires"] = "%d" % lot.expires
    return base


def shipment_label(shipment):
    if shipment.label is None:
        return {"key": key_for("ship", shipment.ship_id), "state": shipment.state}
    out = dict(shipment.label)
    out["key"] = key_for("ship", shipment.ship_id)
    out["state"] = shipment.state
    return out


def code_for(record):
    parts = [str(record[k]) for k in sorted(record) if not isinstance(record[k], (dict, list))]
    text = "|".join(parts)
    return {"code": text, "n": len(parts)}


def parse_labels(labels):
    out = {}
    for label in labels:
        parsed = parse_key(label["key"])
        out[parsed["ident"]] = parsed["kind"]
    return out

def normalize(rec):
    out = {k.lower(): v for k, v in rec.items()}
    out["normalized"] = True
    return out


DEFAULTS = {"retries": 3, "timeout": 10}

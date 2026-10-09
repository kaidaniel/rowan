"""Layered configuration: defaults, a named profile, explicit overrides; dotted access with
a read log; validation that produces a report dict; sections copied out."""
from .util.paths import deep_get, deep_merge, flatten

DEFAULTS = {
    "site": {"name": "main", "timezone": "UTC"},
    "picking": {"strategy": "fifo", "wave_size": 3, "zones": ["A", "B", "C"]},
    "shipping": {"default_carrier": "dhl", "free_over_g": 20000, "insure_over": 50000},
    "receiving": {"require_lot": True, "quarantine_days": 2},
    "features": {"audit": False, "fast_track": True, "cold_chain": False},
    "limits": {"max_boxes": 4, "box_weight_g": 10000},
}

PROFILES = {
    "cold": {"features": {"cold_chain": True}, "receiving": {"quarantine_days": 0}},
    "small": {"limits": {"max_boxes": 2}, "picking": {"wave_size": 1}},
}


class Config:
    def __init__(self, data, profile=None):
        self.data = data
        self.profile = profile
        self.reads = []
        self.misses = {}

    def get(self, path, default=None):
        hit = deep_get(self.data, path, None)
        if hit is None:
            self.misses[path] = self.misses.get(path, 0) + 1
            self.reads.append({"path": path, "hit": False})
            return default
        self.reads.append({"path": path, "hit": True})
        return hit

    def feature(self, name):
        return bool(self.get("features." + name, False))

    def section(self, name):
        sec = self.data.get(name)
        if sec is None:
            return {}
        return dict(sec)

    def flat(self):
        return flatten(self.data)

    def describe(self):
        d = {"site": self.get("site.name"), "profile": self.profile, "reads": len(self.reads)}
        if self.misses:
            d["misses"] = dict(self.misses)
        return d


def build_config(profile=None, overrides=None):
    data = DEFAULTS
    if profile is not None and profile in PROFILES:
        data = deep_merge(data, PROFILES[profile])
    if overrides:
        data = deep_merge(data, overrides)
    return Config(data, profile)


def validate(cfg):
    report = {"ok": True, "problems": []}
    zones = cfg.get("picking.zones", [])
    if not zones:
        report["ok"] = False
        report["problems"].append({"path": "picking.zones", "why": "empty"})
    strategy = cfg.get("picking.strategy")
    if strategy not in ("fifo", "nearest", "zone"):
        report["ok"] = False
        report["problems"].append({"path": "picking.strategy", "why": "unknown"})
    if cfg.feature("cold_chain") and not cfg.get("receiving.quarantine_days") == 0:
        report["problems"].append({"path": "receiving.quarantine_days", "why": "cold chain needs 0"})
        report["warn"] = True
    report["checked"] = ["picking.zones", "picking.strategy", "cold_chain"]
    return report

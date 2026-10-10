"""Configuration: a literal INI text, then REPORT_* environment overrides."""
import configparser
import os

INI = """
[report]
title = Monthly sales
currency = EUR
min_qty = 1
regions = north,south

[output]
width = 40
format = table
"""

INI_WITH_FOOTER = INI.replace("regions = north,south", "regions = north,south,east\nfooter = internal use only")


def load_config(text=INI):
    cp = configparser.ConfigParser()
    cp.read_string(text)
    report = cp["report"]
    cfg = {
        "title": report["title"],
        "currency": report.get("currency", "USD"),
        "min_qty": cp.getint("report", "min_qty"),
        "regions": [r.strip() for r in report["regions"].split(",")],
        "width": cp.getint("output", "width"),
        "format": cp.get("output", "format", fallback="table"),
    }
    if cp.has_option("report", "footer"):
        cfg["footer"] = report["footer"]
    return cfg


def apply_env(cfg, prefix="REPORT_"):
    out = dict(cfg)
    overridden = []
    title = os.environ.get(prefix + "TITLE")
    if title is not None:
        out["title"] = title
        overridden.append("title")
    if prefix + "WIDTH" in os.environ:
        out["width"] = int(os.environ[prefix + "WIDTH"])
        overridden.append("width")
    out["overridden"] = overridden
    return out

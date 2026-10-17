"""A function that copies a global dict (`dict(DEFAULT_CONFIG)`), updates the copy and stores
into it changes the copy, not the global's values; its caller reading one value of it
(`cfg["output"]`) and giving that away gives away that value only. For any call, the global
list `DEFAULT_CONFIG["regions"]` was `list[?]`: the copy was taken for one of the global's
parts, so its changes were changes of what the global holds, and any key read of it any part.

Expectations were written from the program text before running the engine."""
import json
import os

DEFAULT_CONFIG = {"min_amount": 10, "regions": ["EU", "US"], "output": "report.csv"}


def load_config(path):
    cfg = dict(DEFAULT_CONFIG)
    if os.path.exists(path):
        with open(path) as f:
            cfg.update(json.load(f))
    cfg["threshold"] = cfg["min_amount"] * 2
    return cfg


def main():
    cfg = load_config(os.environ.get("Z655_CFG", "/nonexistent/z655.json"))
    print(cfg["output"].upper(), cfg["threshold"])


def regions_now():
    rs = DEFAULT_CONFIG["regions"]
    n = len(rs)
    #@general n: exactly={2}
    first = rs[0]
    #@general first: exactly={"EU"}
    return n


main()
print(regions_now())

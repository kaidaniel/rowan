def add_defaults(d):
    d.setdefault("retries", 3)
    if "timeout" not in d:
        d["timeout"] = 10
    return d


def strip_secret(d):
    d.pop("secret", None)


cfg = {"url": "x", "secret": "s"}
add_defaults(cfg)
cfg["retries"]                                  #@ ok retries
cfg["timeout"]                                  #@ ok timeout
strip_secret(cfg)
cfg["secret"]                                   #@ missing secret
cfg["url"]                                      #@ ok url

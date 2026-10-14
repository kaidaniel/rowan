# a module of a namespace package (no __init__.py anywhere under nsapp/)
DEFAULTS = {"host": "localhost", "port": 8080}


def load(overrides):
    cfg = dict(DEFAULTS)
    cfg.update(overrides)
    return cfg

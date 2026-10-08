"""Layered configuration: defaults, a file (open input), environment overrides, merged with
`{**a, **b}`, `update`, `pop` and conditional keys.

Expectations were written from the program text before running the engine."""
import json
import os
import sys

DEFAULTS = {"host": "localhost", "port": 8080, "debug": False, "workers": 2}


def from_file(path):
    with open(path) as fp:
        return json.load(fp)


def from_env():
    env = {}
    if os.environ.get("APP_PORT"):
        env["port"] = int(os.environ["APP_PORT"])
    if "APP_DEBUG" in os.environ:
        env["debug"] = True
    return env


def merge(*layers):
    out = {}
    for layer in layers:
        out.update(layer)
    return out


def finalize(cfg, cli):
    cfg = dict(cfg)
    cfg["cli"] = cli
    secret = cfg.pop("secret", None)
    if secret is not None:
        cfg["has_secret"] = True
    if cfg["debug"]:
        cfg["log_level"] = "DEBUG"
    return cfg


env = from_env()                          #@row env: present={port,debug} definite={} closed
base = merge(DEFAULTS, env)               #@row base: present={host,port,debug,workers} definite={host,port,debug,workers} closed
# The file layer is an external input: everything stays open after merging it in.
full = merge(DEFAULTS, from_file(sys.argv[1]), env)  #@row full: present={host,port,debug,workers} definite={host,port,debug,workers} open
cfg = finalize(base, ["--x"])             #@row cfg: present={host,port,debug,workers,cli,log_level} definite={host,port,debug,workers,cli} closed
cfg["log_level"]                          #@ possibly log_level
cfg["has_secret"]                         #@ missing has_secret
cfg2 = finalize({"host": "h", "port": 1, "debug": True, "workers": 1, "secret": "s"}, [])  #@row cfg2: present={host,port,debug,workers,cli,has_secret,log_level} definite={host,port,debug,workers,cli,has_secret,log_level} closed
cfg2["secret"]                            #@ missing secret
spread = {**DEFAULTS, **env, "extra": 1}  #@row spread: present={host,port,debug,workers,extra} definite={host,port,debug,workers,extra} closed
full["timeout"]                           #@ input timeout
#@schema json.load@L14: always={timeout}

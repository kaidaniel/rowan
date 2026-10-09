"""Model: layered application settings. Built-in defaults < INI file (configparser, DEFAULT
section inheritance, interpolation, getint / fallback) < environment variables with a prefix
(os.environ: open) < explicit overrides from a caller; merged with ChainMap and dict `|`, turned
into a dataclass and back into a dict (asdict)."""
import configparser
import os
from collections import ChainMap
from dataclasses import asdict, dataclass, field

BUILTIN = {"host": "127.0.0.1", "port": "8000", "workers": "1", "log_level": "info"}
KNOWN = ("host", "port", "workers", "log_level")
PREFIX = "SM02_"

INI_BASE = """
[DEFAULT]
log_level = warning

[app]
host = 0.0.0.0
port = 9000
url = http://%(host)s:%(port)s

[db]
url = sqlite:///x.db
pool = 5
"""

INI_MIN = """
[app]
workers = 4
"""


@dataclass
class Settings:
    host: str
    port: int
    workers: int
    log_level: str
    extras: dict = field(default_factory=dict)


def read_ini(text):
    cp = configparser.ConfigParser()
    cp.read_string(text)
    return cp


def section_dict(cp, name):
    if not cp.has_section(name):
        return {}
    d = dict(cp[name])
    _ = 0                                        #@row d: present={host,port,url,log_level,workers} closed
    return d


def all_sections(cp):
    out = {s: dict(cp[s]) for s in cp.sections()}
    _ = 0                                        #@row out: present={app,db} definite={app} closed
    app = out["app"]                             #@ ok app
    return out


def env_overrides(prefix):
    out = {}
    for key, value in os.environ.items():
        if key.startswith(prefix):
            out[key[len(prefix):].lower()] = value
    _ = 0                                        #@row out: open
    return out


def layered(cp):
    app = section_dict(cp, "app")
    env = env_overrides(PREFIX)
    chain = ChainMap(env, app, BUILTIN)
    chain["host"]                                #@ ok host
    chain["port"]                                #@ ok port
    _ = 0                                        #@row chain: definite={host,port,workers,log_level} open
    n = len(chain.maps)                          #@value n: exactly={3}
    return chain


def to_settings(chain):
    s = Settings(
        host=chain["host"],
        port=int(chain["port"]),
        workers=int(chain["workers"]),
        log_level=chain["log_level"],
    )
    for k in chain:
        if k not in KNOWN:
            s.extras[k] = chain[k]
    _ = 0                                        #@row s: present={host,port,workers,log_level,extras} definite={host,port,workers,log_level,extras} closed
    ex = s.extras                                #@row ex: open
    return s


def db_config(cp):
    if "db" not in cp:
        return None
    sec = cp["db"]                               #@ ok db
    #@row sec: present={url,pool,log_level} definite={url,pool,log_level} closed
    url = sec["url"]                             #@ ok url
    pool = sec.getint("pool", fallback=1)        #@value pool: exactly={5}
    timeout = sec.getfloat("timeout", fallback=30.0)  #@value timeout: exactly={30.0}
    return {"url": url, "pool": pool, "timeout": timeout}


def apply_overrides(cfg, raw):
    out = dict(cfg)
    for key in ("host", "port"):
        if key in raw:
            out[key] = raw[key]                  #@ ok host ; ok port
    out["workers"] = int(raw.get("workers", out["workers"]))  #@ none workers
    if raw["strict"]:                            #@ ok strict
        out["strict"] = True
    _ = 0                                        #@row out: present={host,port,workers,log_level,extras,db,strict} definite={host,port,workers,log_level,extras} closed
    return out
#@schema param apply_overrides.raw: always={strict} only={strict}


def build(text, overrides=None):
    cp = read_ini(text)
    secs = tuple(cp.sections())                  #@value secs: exactly={("app","db"),("app",)}
    all_sections(cp)
    chain = layered(cp)
    s = to_settings(chain)
    cfg = asdict(s)                              #@row cfg: present={host,port,workers,log_level,extras} definite={host,port,workers,log_level,extras} closed
    db = db_config(cp)
    if db is not None:
        cfg["db"] = db
    if overrides is not None:
        cfg = apply_overrides(cfg, overrides)
    _ = 0                                        #@row cfg: present={host,port,workers,log_level,extras,db,strict} definite={host,port,workers,log_level,extras} closed
    return cfg


def mode():
    os.environ.setdefault("SM02_MODE", "dev")
    m = os.environ["SM02_MODE"]                  #@ ok SM02_MODE
    return m


def secret():
    s = os.environ.get("SM02_SECRET")            #@ none SM02_SECRET
    if s is None:
        return os.environ["SM02_FALLBACK"]       #@ input SM02_FALLBACK
    return s


def db_url(cfg):
    return cfg["db"]["url"]                      #@ ok db ; missing db


def set_env(values):
    for k in [k for k in os.environ if k.startswith(PREFIX)]:
        del os.environ[k]
    os.environ.update(values)


if __name__ == "__main__":
    set_env({"SM02_PORT": "7000", "SM02_DEBUG": "1", "SM02_FALLBACK": "fb"})
    c1 = build(INI_BASE)
    p1 = c1["port"]                              #@value p1: exactly={7000}
    ex1 = c1["extras"]                           #@row ex1: definite={url} open
    # precise: the driver controls every SM02_ variable (set_env deletes the others)
    #@todo row ex1: present={debug,fallback,url} definite={debug,fallback,url} closed
    s1 = secret()                                #@value s1: exactly={fb}
    m1 = mode()                                  #@value m1: exactly={dev}

    set_env({})
    c2 = build(INI_MIN, {"strict": False, "workers": "8"})
    w2 = c2["workers"]                           #@value w2: exactly={8}
    try:
        secret()
    except KeyError:
        pass

    set_env({"SM02_SECRET": "s", "SM02_MODE": "prod"})
    c3 = build(INI_BASE, {"strict": True, "host": "h"})
    h3 = c3["host"]                              #@value h3: exactly={h}
    m3 = mode()                                  #@value m3: exactly={prod}
    u = read_ini(INI_BASE)["app"]["url"]         #@value u: exactly={"http://0.0.0.0:9000"}
    for c in (c1, c2, c3):
        try:
            db_url(c)
        except KeyError:
            pass

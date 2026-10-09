# configuration: BaseSettings from prefixed env vars, layered dict configs deep-merged in functions and validated into frozen nested models, feature flags as a RootModel
import copy
import os

from pydantic import BaseModel, ConfigDict, Field, RootModel, ValidationError, field_validator
from pydantic_settings import BaseSettings, SettingsConfigDict


class DbConfig(BaseModel):
    model_config = ConfigDict(frozen=True)
    url: str
    pool_size: int = 5
    timeout: float = 30.0


class CacheConfig(BaseModel):
    model_config = ConfigDict(frozen=True)
    backend: str = "memory"
    ttl: int = 60


class Flags(RootModel[dict[str, bool]]):
    def enabled(self, name):
        return self.root.get(name, False)


class AppConfig(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid")
    name: str
    env: str = "dev"
    db: DbConfig
    cache: CacheConfig = CacheConfig()
    flags: Flags = Flags({})
    workers: int = Field(1, ge=1)

    @field_validator("env")
    @classmethod
    def known_env(cls, v):
        if v not in ("dev", "staging", "prod"):
            raise ValueError("unknown env")
        return v


class EnvSettings(BaseSettings):
    model_config = SettingsConfigDict(env_prefix="ROWCFG_", env_nested_delimiter="__")
    env: str = "dev"
    workers: int = 1
    db_url: str | None = None
    secret_key: str


DEFAULTS = {"name": "svc", "env": "dev", "db": {"url": "sqlite://", "pool_size": 5}, "cache": {"backend": "memory"}}


def deep_merge(base, override):
    out = copy.deepcopy(base)
    for k, v in override.items():
        if isinstance(v, dict) and isinstance(out.get(k), dict):
            out[k] = deep_merge(out[k], v)
        else:
            out[k] = v
    return out


def from_env():
    s = EnvSettings()                            #@ possibly secret_key
    #@row s: present={env,workers,db_url,secret_key} definite={env,workers,db_url,secret_key} closed
    layer = {"env": s.env, "workers": s.workers}
    if s.db_url:
        layer["db"] = {"url": s.db_url}
    _ = 0  #@row layer: present={env,workers,db} definite={env,workers} closed
    return layer


def build(*layers):
    merged = DEFAULTS
    for layer in layers:
        merged = deep_merge(merged, layer)
    _ = 0  #@row merged: present={name,env,db,cache,workers,flags,debug} definite={name,env,db,cache} closed
    cfg = AppConfig.model_validate(merged)
    #@row cfg: present={name,env,db,cache,flags,workers} definite={name,env,db,cache,flags,workers} closed
    db = cfg.db
    #@row db: present={url,pool_size,timeout} definite={url,pool_size,timeout} closed
    return cfg


def describe(cfg):
    d = cfg.model_dump(exclude={"flags"})
    #@row d: present={name,env,db,cache,workers} definite={name,env,db,cache,workers} absent={flags} closed
    dd = d["db"]                                 #@ ok db
    #@row dd: present={url,pool_size,timeout} definite={url,pool_size,timeout} closed
    c = d["cache"]
    #@row c: present={backend,ttl} definite={backend,ttl} closed
    return f"{d['name']}@{d['env']} db={dd['url']} cache={c['backend']}"   #@ ok name ; ok env ; ok url ; ok backend


def scale(cfg, workers):
    try:
        cfg.workers = workers
    except ValidationError:
        pass
    bigger = cfg.model_copy(update={"workers": workers})
    #@row bigger: present={name,env,db,cache,flags,workers} definite={name,env,db,cache,flags,workers} closed
    return bigger


def flag_names(cfg):
    fl = cfg.flags.root
    return sorted(k for k, v in fl.items() if v)


def documented_keys():
    props = AppConfig.model_json_schema()["properties"]
    #@row props: present={name,env,db,cache,flags,workers} definite={name,env,db,cache,flags,workers} closed
    return sorted(props)


for k in [k for k in os.environ if k.upper().startswith("ROWCFG_")]:
    del os.environ[k]
os.environ["ROWCFG_SECRET_KEY"] = "s3cret"
os.environ["ROWCFG_WORKERS"] = "4"

env_layer = from_env()
w = env_layer["workers"]
#@value w: exactly={4}
base = build(env_layer)
bw = base.workers
#@value bw: exactly={4}
pool = base.db.pool_size
#@value pool: exactly={5}
ttl = base.cache.ttl
#@value ttl: exactly={60}
prod = build({"env": "prod", "db": {"pool_size": 20}, "flags": {"beta": True, "dark": False}})
purl = prod.db.url
#@value purl: exactly={"sqlite://"}
ppool = prod.db.pool_size
#@value ppool: exactly={20}
beta = prod.flags.enabled("beta")
#@value beta: exactly={True}
nf = len(flag_names(prod))
#@value nf: exactly={1}
text = describe(prod)
#@value text: exactly={"svc@prod db=sqlite:// cache=memory"}
big = scale(prod, 8)
still = prod.workers
#@value still: exactly={1}
bwk = big.workers
#@value bwk: exactly={8}

if __name__ == "__main__":
    describe(base)
    documented_keys()
    os.environ["ROWCFG_DB_URL"] = "postgres://db"
    os.environ["ROWCFG_ENV"] = "staging"
    staged = build(from_env(), {"cache": {"ttl": 5}})
    describe(staged)
    for bad in ({"env": "qa"}, {"db": {"pool_size": 1, "url": 3}}, {"debug": True}):
        try:
            build(bad)
        except ValidationError:
            pass
    del os.environ["ROWCFG_SECRET_KEY"]
    try:
        from_env()
    except ValidationError:
        pass

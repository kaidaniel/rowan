"""Hypothesis: `d.pop(k, None) or default` falls back on every falsy stored value (0, "", []),
not only on a missing key, and removes the key either way. Precise: norm of {name: "", tags: [],
level: 0} gives name "anon", ntags 0, level 1, rest ["level"], and leaves cfg {level}."""
import json
import os


def take(d, k, default):
    return d.pop(k, None) or default


def norm(cfg):
    name = cfg.pop("name", None) or "anon"
    tags = cfg.pop("tags", None) or []
    level = cfg.get("level") or 1
    return {"name": name, "ntags": len(tags), "level": level, "rest": sorted(cfg)}


if __name__ == "__main__":
    cfg = {"name": "", "tags": [], "level": 0}
    r = norm(cfg)
    #@row r: present={name,ntags,level,rest} definite={name,ntags,level,rest} closed
    nm = r["name"]                               #@value nm: exactly={"anon"}
    lv = r["level"]                              #@value lv: exactly={1}
    _ = 0                                        #@row cfg: present={level} definite={level} absent={name,tags} closed
    r2 = norm({"name": "bo", "tags": ["a"]})
    nm2 = r2["name"]                             #@value nm2: exactly={"bo"}
    nt2 = r2["ntags"]                            #@value nt2: exactly={1}
    d = {"n": 0, "m": 5}
    a = take(d, "n", 9)                          #@value a: exactly={9}
    b = take(d, "m", 9)                          #@value b: exactly={5}
    _ = 0                                        #@row d: present={} definite={} absent={n,m} closed
    u = json.loads(os.environ.get("Z436_U", '{"name": "x", "k": 1}'))
    ru = norm(u)
    #@row u: absent={name,tags} open
    nmu = ru["name"]                             #@ ok name

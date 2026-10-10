"""Hypothesis: string helpers called from other functions produce exact dict keys on literal
text: split + filter + join over a generator, partition (with and without the separator),
f-strings with format specs (`{i:02d}`, `{x:07.2f}`, slicing), upper / replace chains. A line
from the environment gives a row whose only known possible key is the no-separator one."""
import os


def slug(title):
    return "-".join(w.lower() for w in title.split() if w.isalnum())


def parse_kv(line):
    head, sep, tail = line.partition(":")
    if not sep:
        return {"raw": line.strip()}
    return {head.strip().lower(): tail.strip()}


def headers(lines):
    return {k: v for ln in lines for k, v in parse_kv(ln).items()}


def keyed(items):
    out = {}
    for i, (name, price) in enumerate(items):
        out[f"{i:02d}_{name[:3]}"] = f"{price:07.2f}"
    return out


def env_key(path):
    return "_".join(p.upper() for p in path.split(".")).replace("-", "_")


def env_table(cfg):
    return {env_key(k): v for k, v in cfg.items()}


if __name__ == "__main__":
    s1 = slug("Hello  Big World")                #@value s1: exactly={"hello-big-world"}
    s2 = slug("A & B 2")                         #@value s2: exactly={"a-b-2"}
    h = parse_kv("Host: example.org")
    #@row h: present={host} definite={host} closed
    hv = h["host"]                               #@value hv: exactly={"example.org"}
    r = parse_kv("  noise ")
    #@row r: present={raw} definite={raw} absent={host} closed
    rv = r["raw"]                                #@value rv: exactly={"noise"}
    hs = headers(["Host: a", "X-Id : 42", "junk"])
    #@row hs: present={host,"x-id",raw} definite={host,"x-id",raw} closed
    xid = hs["x-id"]                             #@ ok "x-id"
    _ = 0                                        #@value xid: exactly={"42"}
    kd = keyed([("apple", 1.5), ("kiwi", 12)])
    #@row kd: present={"00_app","01_kiw"} definite={"00_app","01_kiw"} closed
    p0 = kd["00_app"]                            #@value p0: exactly={"0001.50"}
    tb = env_table({"db.host": "h", "db.port-no": 5})
    #@row tb: present={DB_HOST,DB_PORT_NO} definite={DB_HOST,DB_PORT_NO} closed
    port = tb["DB_PORT_NO"]                      #@value port: exactly={5}
    e = parse_kv(os.environ.get("LINE", "Accept: text/plain"))
    #@row e: present={raw} definite={} open

# int / float / str / bool conversions of record values; int() of bad text raises ValueError


def parse_port(cfg):
    raw = cfg.get("port", "8080")
    try:
        port = int(raw)
    except ValueError:
        port = -1
    return port


def coerce(rec):
    out = dict(rec)
    if "qty" in out:
        out["qty"] = int(out["qty"])
    out["price"] = float(out.get("price", 0))
    _ = 0                                        #@row out: present={sku,qty,price} definite={sku,price} closed
    out["price"]                                 #@ ok price
    return out


if __name__ == "__main__":
    p1 = parse_port({"port": "80"})              #@value p1: exactly={80}
    p2 = parse_port({})                          #@value p2: exactly={8080}
    p3 = parse_port({"port": "x"})               #@value p3: exactly={-1}
    coerce({"sku": "a", "qty": "3"})
    c = coerce({"sku": "b", "price": "2.5"})
    v = c["price"]                               #@value v: exactly={2.5}
    a = int("0x1f", 16)                          #@value a: exactly={31}
    b = int(" 7 ")                               #@value b: exactly={7}
    f = float("1e3")                             #@value f: exactly={1000.0}
    s = str(3.0)                                 #@value s: exactly={"3.0"}
    e = bool("")                                 #@value e: exactly={False}
    z = bool("0")                                #@value z: exactly={True}
    t = int(-3.9)                                #@value t: exactly={-3}

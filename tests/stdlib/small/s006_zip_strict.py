# zip(..., strict=True) raises ValueError on a length mismatch; zip of literal keys builds closed rows

HEADER = ["sku", "qty", "price"]


def to_record(cells):
    rec = dict(zip(HEADER, cells, strict=True))
    _ = 0                                        #@row rec: present={sku,qty,price} definite={sku,qty,price} closed
    rec["price"]                                 #@ ok price
    return rec


def loose(cells):
    rec = dict(zip(HEADER, cells))
    _ = 0                                        #@row rec: present={sku,qty,price} definite={sku} closed
    q = rec.get("qty", 0)                        #@ none qty
    return rec


def load(lines):
    out = []
    bad = 0
    for line in lines:
        try:
            out.append(to_record(line.split(",")))
        except ValueError:
            bad += 1
    return out, bad


if __name__ == "__main__":
    recs, bad = load(["a,1,2.5", "b,2", "c,3,1.0,extra"])
    #@value bad: exactly={2}
    n = len(recs)                                #@value n: exactly={1}
    loose(["a", "1", "2"])
    loose(["b"])
    pairs = tuple(zip("ab", [1, 2], strict=True)) #@value pairs: exactly={(("a",1),("b",2))}

"""sqlite3 through helper functions on a module-level connection: GROUP BY with SUM / COUNT /
ROUND / AVG / TOTAL / MIN / MAX, expressions and aliases in the SELECT list, ORDER BY an alias /
a position / rowid, LIMIT ? OFFSET, UPDATE SET qty = qty + ?, rowcount, an UPDATE that matches
nothing inside a try; HAVING leaves the rows unknown but the columns known."""
import sqlite3

conn = sqlite3.connect(":memory:")
conn.execute("CREATE TABLE items (sku TEXT PRIMARY KEY, name TEXT, qty INTEGER, price REAL, category TEXT)")
conn.executemany("INSERT INTO items VALUES (?, ?, ?, ?, ?)", [("A1", "bolt", 40, 0.25, "hardware"), ("B2", "nut", 2, 0.1, "hardware"), ("C3", "glue", 1, 4.5, "supplies"), ("D4", "tape", 3, None, "supplies")])
conn.row_factory = sqlite3.Row


def q(sql, *ps):
    return [dict(r) for r in conn.execute(sql, ps)]


def bump(sku, delta):
    cur = conn.execute("UPDATE items SET qty = qty + ? WHERE sku = ?", (delta, sku))
    if cur.rowcount == 0:
        raise LookupError(sku)
    return cur.rowcount


if __name__ == "__main__":
    n = bump("B2", 5)                            #@value n: exactly={1}
    try:
        bump("Z9", 1)
    except LookupError:
        pass
    top = q("SELECT category, SUM(qty) AS units, ROUND(SUM(qty * price), 2) AS value, COUNT(*) AS n FROM items GROUP BY category ORDER BY value DESC LIMIT ?", 1)
    t0 = top[0]                                  #@row t0: present={category,units,value,n} definite={category,units,value,n} closed
    units = t0["units"]                          #@value units: exactly={47}
    value = t0["value"]                          #@value value: exactly={10.7}
    agg = q("SELECT COUNT(*) AS c, MAX(qty) AS mx, MIN(name) AS mn, AVG(qty) AS av, TOTAL(price) AS tp FROM items")[0]
    av = agg["av"]                               #@value av: exactly={12.75}
    tp = agg["tp"]                               #@value tp: exactly={4.85}
    mn = agg["mn"]                               #@value mn: exactly={"bolt"}
    tags = tuple(r["tag"] for r in q("SELECT name || '-' || sku AS tag FROM items WHERE qty > ? ORDER BY 1", 2))
    _ = 0                                        #@value tags: exactly={("bolt-A1","nut-B2","tape-D4")}
    second = q("SELECT sku FROM items ORDER BY qty DESC LIMIT 1 OFFSET 1")[0]["sku"]   #@value second: exactly={"B2"}
    last = q("SELECT sku FROM items ORDER BY rowid DESC")[0]["sku"]   #@value last: exactly={"D4"}
    hv = q("SELECT category, COUNT(*) AS c FROM items GROUP BY category HAVING COUNT(*) > 1")
    for h in hv:
        _ = 0                                    #@row h: present={category,c} definite={category,c} closed

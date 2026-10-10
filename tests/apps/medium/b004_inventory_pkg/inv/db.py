"""sqlite3 in-memory storage; rows come back as dicts keyed by the SELECT's columns."""
import sqlite3

SCHEMA = """
CREATE TABLE items (sku TEXT PRIMARY KEY, name TEXT, qty INTEGER, price REAL, category TEXT);
CREATE TABLE moves (sku TEXT, delta INTEGER, reason TEXT);
"""

SEED = [("A1", "bolt", 40, 0.25, "hardware"), ("B2", "nut", 2, 0.1, "hardware"), ("C3", "glue", 1, 4.5, "supplies")]


def connect():
    conn = sqlite3.connect(":memory:")
    conn.row_factory = sqlite3.Row
    conn.executescript(SCHEMA)
    conn.executemany("INSERT INTO items VALUES (?, ?, ?, ?, ?)", SEED)
    return conn


def query(conn, sql, params=()):
    return [dict(r) for r in conn.execute(sql, params)]


def move(conn, sku, delta, reason):
    cur = conn.execute("UPDATE items SET qty = qty + ? WHERE sku = ?", (delta, sku))
    if cur.rowcount == 0:
        raise LookupError(sku)
    conn.execute("INSERT INTO moves VALUES (?, ?, ?)", (sku, delta, reason))
    return query(conn, "SELECT sku, qty FROM items WHERE sku = ?", (sku,))[0]

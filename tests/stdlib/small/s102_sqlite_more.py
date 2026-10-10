# sqlite3: a database file is an input (selected columns are the row keys), inserts in a loop, cursor iteration, a dict row factory, count(*), fetchone past the end
import sqlite3


def report(path):
    con = sqlite3.connect(path)
    con.row_factory = sqlite3.Row
    out = []
    for r in con.execute("SELECT sku, qty AS n FROM stock WHERE qty > ?", (0,)):
        #@row r: present={sku,n} definite={sku,n} closed
        out.append({"sku": r["sku"], "n": r["n"]})   #@ ok n
    return out


def dict_factory(cursor, row):
    return {d[0]: v for d, v in zip(cursor.description, row)}


def load(items):
    con = sqlite3.connect(":memory:")
    con.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT, score REAL)")
    for name, score in items:
        con.execute("INSERT INTO t (name, score) VALUES (?, ?)", (name, score))
    return con


con = sqlite3.connect(":memory:")
con.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT, score REAL)")
con.executemany("INSERT INTO t (name, score) VALUES (?, ?)", [("a", 1), ("b", 2.5), ("c", 2)])
n = con.execute("SELECT count(*) FROM t").fetchone()[0]          #@value n: exactly={3}
top = con.execute("SELECT name FROM t ORDER BY score DESC LIMIT 1").fetchone()
tn = top[0]                                                         #@value tn: exactly={"b"}
sc = con.execute("SELECT score FROM t WHERE name = ?", ("a",)).fetchone()[0]   #@value sc: exactly={1.0}
cur = con.execute("SELECT id FROM t WHERE score >= 2")
first = cur.fetchone()
second = cur.fetchone()
third = cur.fetchone()                                              #@value third: exactly={None}
ids = [r[0] for r in con.execute("SELECT id FROM t ORDER BY id")]
i2 = ids[2]                                                         #@value i2: exactly={3}
con.row_factory = dict_factory
d = con.execute("SELECT id, name FROM t WHERE id = 2").fetchone()  #@row d: present={id,name} definite={id,name} closed
dn = d["name"]                                                      #@value dn: exactly={"b"}
con.execute("UPDATE t SET score = ? WHERE name = ?", (9, "c"))
up = con.execute("SELECT score FROM t WHERE id = 3").fetchone()["score"]   #@value up: exactly={9.0}
con.execute("DELETE FROM t WHERE score < 2")
left = len(con.execute("SELECT * FROM t").fetchall())               #@value left: exactly={2}
other = load([("x", 1), ("y", 2)])
rows = other.execute("SELECT name, score FROM t").fetchall()
nm = rows[0][0]                                                     #@value nm: exactly={"x"}
import os
import tempfile
path = os.path.join(tempfile.mkdtemp(), "stock.db")
setup_con = sqlite3.connect(path)
setup_con.execute("CREATE TABLE stock (sku TEXT, qty INTEGER)")
setup_con.executemany("INSERT INTO stock VALUES (?, ?)", [("p", 2), ("q", 0)])
setup_con.commit()
setup_con.close()
out = report(path)

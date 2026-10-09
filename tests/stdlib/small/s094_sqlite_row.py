# sqlite3 with row_factory = sqlite3.Row on an in-memory database: rows are keyed by the selected columns

import sqlite3


def setup():
    con = sqlite3.connect(":memory:")
    con.row_factory = sqlite3.Row
    con.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, email TEXT)")
    con.executemany("INSERT INTO users (name, email) VALUES (?, ?)", [("ann", "a@x"), ("bob", None)])
    return con


def first_user(con):
    row = con.execute("SELECT id, name FROM users ORDER BY id").fetchone()
    #@row row: present={id,name} definite={id,name} closed
    name = row["name"]                            #@ ok name
    return dict(row)


def contacts(con):
    rows = con.execute("SELECT name, email AS mail FROM users").fetchall()
    out = [{"who": r["name"], "mail": r["mail"]} for r in rows]   #@ ok mail
    return out


def to_dicts(cur):
    cols = [d[0] for d in cur.description]
    return [dict(zip(cols, t)) for t in cur.fetchall()]


con = setup()
u = first_user(con)                               #@row u: present={id,name} definite={id,name} closed
cs = contacts(con)
c0 = cs[0]                                        #@row c0: present={who,mail} definite={who,mail} closed
r = con.execute("SELECT * FROM users WHERE id = ?", (2,)).fetchone()
#@row r: present={id,name,email} definite={id,name,email} closed
nk = len(r.keys())                                #@value nk: exactly={3}
con.row_factory = None
ds = to_dicts(con.execute("SELECT id AS uid, email FROM users"))
d0 = ds[0]                                        #@row d0: present={uid,email} definite={uid,email} closed

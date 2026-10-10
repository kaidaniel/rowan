"""Subcommands, registered by name through a wrapping decorator; each takes (conn, cfg, args)
and returns a result dict, to which the wrapper adds the command's name."""
from functools import wraps

from inv.db import move, query

COMMANDS = {}


def command(name):
    def deco(fn):
        @wraps(fn)
        def run(conn, cfg, args):
            res = fn(conn, cfg, args)
            res["command"] = name
            return res
        COMMANDS[name] = run
        return run
    return deco


@command("add")
def cmd_add(conn, cfg, args):
    row = move(conn, args.sku, args.qty, args.reason or "restock")
    #@row row: present={sku,qty} definite={sku,qty} closed
    return {"sku": row["sku"], "qty": row["qty"]}


@command("low")
def cmd_low(conn, cfg, args):
    limit = int(cfg["low_stock"])                #@ ok low_stock
    rows = query(conn, "SELECT sku, name, qty FROM items WHERE qty < ? ORDER BY qty", (limit,))
    return {"limit": limit, "items": rows}


@command("report")
def cmd_report(conn, cfg, args):
    top = args.top if args.top is not None else int(cfg["top"])
    rows = query(conn, "SELECT category, SUM(qty) AS units, ROUND(SUM(qty * price), 2) AS value, COUNT(*) AS n "
                       "FROM items GROUP BY category ORDER BY value DESC LIMIT ?", (top,))
    for r in rows:
        r["currency"] = cfg["currency"]          #@row r: present={category,units,value,n,currency} definite={category,units,value,n,currency} closed
    out = {"title": cfg["title"], "rows": rows}
    if args.verbose:
        out["moves"] = query(conn, "SELECT * FROM moves")
    return out


def apply_moves(conn, payload):
    done = []
    for m in payload["moves"]:
        sku = m["sku"]                           #@ ok sku ; input sku
        done.append(move(conn, sku, int(m["delta"]), m.get("reason", "sync")))
    return done
#@schema param apply_moves.payload: always={moves} only={moves}

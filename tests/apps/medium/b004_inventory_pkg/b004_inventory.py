# Inventory CLI over a package: argparse subcommands dispatched through a registry filled by a wrapping decorator (functools.wraps / partial), configuration from configparser + os.environ + defaults in a ChainMap, sqlite3 in-memory storage with sqlite3.Row rows, argv and JSON moves from literals and from the environment
"""Hypothesis: the result of main(argv) is the row of the one subcommand the argv selects (plus
the "command" key the registry's wrapper adds) when argv is literal, and the closed join of all
subcommands' rows when argv comes from the environment. Rows read back from sqlite are keyed
by the SELECT's columns (aliases, `*` expanded from the CREATE TABLE); ChainMap lookups see the
environment layer only for the keys the driver set, after popping them first."""
import json
import os

from inv.cli import main
from inv.commands import COMMANDS, apply_moves
from inv.config import ALT_INI, load_config, region_of
from inv.db import connect, query

if __name__ == "__main__":
    for k in ("INV_LOW_STOCK", "INV_CURRENCY", "INV_TITLE"):
        os.environ.pop(k, None)
    cfg = load_config()
    flat = dict(cfg)
    #@row flat: present={low_stock,currency,top,title} definite={low_stock,currency,top,title} closed
    cur = cfg["currency"]                        #@value cur: exactly={EUR}
    title = cfg["title"]                         #@value title: exactly={Stock}
    names = tuple(sorted(COMMANDS))              #@value names: exactly={("add","low","report")}
    try:
        region_of(cfg)
    except KeyError:
        pass

    conn = connect()
    added = main(["add", "B2", "--qty", "5"], conn=conn)
    #@row added: present={sku,qty,command} definite={sku,qty,command} closed
    q = added["qty"]                             #@value q: exactly={7}
    low = main(["low"], conn=conn)
    #@row low: present={limit,items,command} definite={limit,items,command} closed
    lim = low["limit"]                           #@value lim: exactly={3}
    nlow = len(low["items"])                     #@value nlow: exactly={1}
    first_low = low["items"][0]
    #@row first_low: present={sku,name,qty} definite={sku,name,qty} absent={price,category} closed
    lsku = first_low["sku"]                      #@value lsku: exactly={C3}
    try:
        main(["add", "Z9"], conn=conn)
    except LookupError:
        pass

    rep = main(["--verbose", "report", "--top", "1"], conn=conn)
    #@row rep: present={title,rows,moves,command} definite={title,rows,moves,command} closed
    nrows = len(rep["rows"])                     #@value nrows: exactly={1}
    r0 = rep["rows"][0]
    cat = r0["category"]                         #@value cat: exactly={hardware}
    units = r0["units"]                          #@value units: exactly={47}
    val = r0["value"]                            #@value val: exactly={10.7}
    mv = rep["moves"][0]
    #@row mv: present={sku,delta,reason} definite={sku,delta,reason} closed
    reason = mv["reason"]                        #@value reason: exactly={restock}

    os.environ["INV_CURRENCY"] = "NOK"
    os.environ["INV_TITLE"] = "Lager"
    rep2 = main(["report"], ini=ALT_INI)
    #@row rep2: present={title,rows,command} definite={title,rows,command} absent={moves} closed
    t2 = rep2["title"]                           #@value t2: exactly={Lager}
    c2 = rep2["rows"][0]["currency"]             #@value c2: exactly={NOK}
    n2 = len(rep2["rows"])                       #@value n2: exactly={2}
    lim2 = main(["low"], ini=ALT_INI)["limit"]   #@value lim2: exactly={1}

    moved = apply_moves(conn, {"moves": [{"sku": "A1", "delta": "-10"}, {"sku": "C3", "delta": 4, "reason": "count"}]})
    m0 = moved[0]                                #@row m0: present={sku,qty} definite={sku,qty} closed
    a1 = m0["qty"]                               #@value a1: exactly={30}
    reasons = tuple(r["reason"] for r in query(conn, "SELECT reason FROM moves ORDER BY rowid"))
    #@value reasons: exactly={("restock","sync","count")}
    payload = json.loads(os.environ.get("INV_MOVES", '{"moves": [{"sku": "B2", "delta": 1}]}'))
    #@schema json.loads@L70['moves']['*']: always={sku,delta} only={sku,delta}
    apply_moves(conn, payload)

    res = main(os.environ.get("INV_ARGS", "report --top 1").split(), conn=conn)
    #@row res: present={sku,qty,limit,items,title,rows,moves,command} definite={command} closed

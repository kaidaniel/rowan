# merge_asof: nearest-key match; columns as merge (on= once, by= once, overlap gets _x / _y)
import pandas as pd

TRADES = pd.DataFrame({"time": [1, 5, 10], "ticker": ["A", "A", "B"], "qty": [1, 2, 3]})
QUOTES = pd.DataFrame({"time": [0, 4, 9], "ticker": ["A", "A", "B"], "bid": [9.0, 9.5, 20.0]})
QUOTES_Q = pd.DataFrame({"time": [0, 9], "ticker": ["A", "B"], "bid": [9.0, 20.0], "qty": [7, 8]})


def nearest(t, q):
    m = pd.merge_asof(t, q, on="time", by="ticker")
    #@row m: present={time,ticker,qty,bid,qty_x,qty_y} definite={time,ticker,bid} closed
    m["bid"]                                             #@ ok bid
    return m


def qty(m):
    return m["qty"]                                      #@ possibly qty


def plain(t, q):
    m = pd.merge_asof(t, q[["time", "bid"]], on="time", direction="forward")
    #@row m: present={time,ticker,qty,bid} definite={time,ticker,qty,bid} closed
    return m


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    qty(nearest(TRADES, QUOTES))
    try:
        qty(nearest(TRADES, QUOTES_Q))
    except KeyError as e:
        kept.append(e)
    plain(TRADES, QUOTES)
    plain(TRADES, QUOTES_Q)

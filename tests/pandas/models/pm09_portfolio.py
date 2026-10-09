"""Financial portfolio: trades + prices merged on date and ticker, positions via cumsum within a
groupby, market value, per-ticker pct_change returns, unstack / pivot by ticker, merge with
suffixes; run on three trade / price datasets (optional fee, account and volume columns,
different tickers)."""
import io

import pandas as pd

TRADES_1 = """date,ticker,side,qty,price,fee,account
2024-04-01,aaa,BUY,10,100.0,1.0,acc1
2024-04-01,bbb,BUY,5,50.0,0.5,acc2
2024-04-02,aaa,SELL,4,102.0,1.0,acc1
2024-04-03,bbb,BUY,5,49.0,0.5,acc1
"""

TRADES_2 = """date,ticker,side,qty,price
2024-04-01,ccc,BUY,100,10.0
2024-04-02,aaa,BUY,3,101.0
2024-04-03,ccc,SELL,40,10.5
"""

PRICES_1 = """date,ticker,close,volume
2024-04-01,AAA,100.0,1000
2024-04-01,BBB,50.0,800
2024-04-02,AAA,102.0,1200
2024-04-02,BBB,51.0,700
2024-04-03,AAA,101.0,900
2024-04-03,BBB,49.0,950
"""

PRICES_2 = """date,ticker,close
2024-04-01,AAA,100.0
2024-04-01,CCC,10.0
2024-04-02,AAA,101.0
2024-04-02,CCC,10.2
2024-04-03,AAA,103.0
2024-04-03,CCC,10.5
"""

SECTORS = pd.DataFrame({"ticker": ["AAA", "BBB", "CCC"], "sector": ["tech", "energy", "retail"]})


def load_trades(text):
    trades = pd.read_csv(io.StringIO(text))
    trades["date"] = pd.to_datetime(trades["date"])                     #@ input date
    trades["ticker"] = trades["ticker"].str.upper()                     #@ input ticker
    sign = trades["side"].map({"BUY": 1, "SELL": -1})                   #@ input side
    trades["signed_qty"] = trades["qty"] * sign                         #@ input qty
    trades["cash_flow"] = -trades["signed_qty"] * trades["price"]       #@ ok signed_qty ; input price
    if "fee" not in trades.columns:
        trades["fee"] = 0.0
    trades["cash_flow"] = trades["cash_flow"] - trades["fee"]           #@ ok fee ; ok cash_flow
    n = len(trades)  #@row trades: present={date,ticker,side,qty,price,signed_qty,cash_flow,fee} definite={date,ticker,side,qty,price,signed_qty,cash_flow,fee} open
    return trades


def daily_trades(trades):
    aggs = {"net_qty": ("signed_qty", "sum"), "cash": ("cash_flow", "sum"), "fees": ("fee", "sum")}
    if "account" in trades.columns:
        aggs["n_accounts"] = ("account", "nunique")
    daily = trades.groupby(["date", "ticker"], as_index=False).agg(**aggs)
    #@row daily: present={date,ticker,net_qty,cash,fees,n_accounts} definite={date,ticker,net_qty,cash,fees} absent={signed_qty,fee} closed
    return daily


def load_prices(text):
    prices = pd.read_csv(io.StringIO(text), parse_dates=["date"])
    prices["ticker"] = prices["ticker"].str.strip()                     #@ input ticker
    prices["close"] = prices["close"].astype(float)                     #@ input close
    if "volume" in prices.columns:
        prices["dollar_vol"] = prices["volume"] * prices["close"]       #@ ok volume ; ok close
    prices = prices.sort_values(["ticker", "date"])
    #@row prices: present={ticker,close,dollar_vol,volume,date} definite={ticker,close,date} open
    return prices


def build_book(prices, daily):
    book = prices[["date", "ticker", "close"]].merge(daily, on=["date", "ticker"], how="left")
    #@row book: present={date,ticker,close,net_qty,cash,fees,n_accounts} definite={date,ticker,close,net_qty,cash,fees} closed
    book = book.fillna({"net_qty": 0, "cash": 0.0, "fees": 0.0})
    book = book.sort_values(["ticker", "date"]).reset_index(drop=True)
    book["position"] = book.groupby("ticker")["net_qty"].cumsum()      #@ ok ticker ; ok net_qty
    book["market_value"] = book["position"] * book["close"]             #@ ok position ; ok close
    book["ret"] = book.groupby("ticker")["close"].pct_change()
    book["growth"] = (1 + book["ret"].fillna(0.0)).groupby(book["ticker"]).cumprod()  #@ ok ret
    book["cash_balance"] = book["cash"].cumsum()                        #@ ok cash
    n = len(book)  #@row book: present={date,ticker,close,net_qty,cash,fees,n_accounts,position,market_value,ret,growth,cash_balance} definite={date,ticker,close,net_qty,cash,fees,position,market_value,ret,growth,cash_balance} closed
    return book


def probe_qty(book):
    # trade quantities were netted into net_qty
    return book["qty"]                                                  #@ missing qty


def probe_accounts(book):
    # only trade files with an account column count accounts
    return book["n_accounts"]                                           #@ possibly n_accounts


def wide_views(book, prices):
    mv = book.set_index(["date", "ticker"])["market_value"].unstack("ticker")  #@ ok market_value
    #@row mv: present={} definite={} open
    mv["total"] = mv.sum(axis=1)
    port = mv[["total"]].copy()                                         #@ ok total
    #@row port: present={total} definite={total} closed
    port["daily_pnl"] = port["total"].diff()                            #@ ok total
    closes = prices.pivot(index="date", columns="ticker", values="close")
    #@row closes: present={} definite={} open
    rets = closes.pct_change().add_suffix("_ret")
    #@row rets: present={} definite={} absent={date,close,total} open
    stacked = rets.stack().rename("ret").reset_index()
    #@row stacked: present={date,ticker,ret} definite={date,ticker,ret} closed
    return port, rets


def snapshot(book, prices):
    last_close = prices.groupby("ticker", as_index=False)["close"].last()
    #@row last_close: present={ticker,close} definite={ticker,close} closed
    snap = book.merge(last_close, on="ticker", suffixes=("", "_last"))
    #@row snap: present={date,ticker,close,close_last,net_qty,cash,fees,n_accounts,position,market_value,ret,growth,cash_balance} definite={date,ticker,close,close_last,net_qty,cash,fees,position,market_value,ret,growth,cash_balance} closed
    snap["mark_to_last"] = snap["position"] * snap["close_last"]        #@ ok close_last
    summary = snap.groupby("ticker").agg(final_pos=("position", "last"), avg_ret=("ret", "mean"), vol=("ret", "std"), mtm=("mark_to_last", "last"))
    #@row summary: present={final_pos,avg_ret,vol,mtm} definite={final_pos,avg_ret,vol,mtm} absent={ticker,position} closed
    summary = summary.reset_index().merge(SECTORS, on="ticker", how="left")
    #@row summary: present={ticker,final_pos,avg_ret,vol,mtm,sector} definite={ticker,final_pos,avg_ret,vol,mtm,sector} closed
    return summary


def report(summary):
    lines = []
    for _, row in summary.iterrows():
        sector = row["sector"]                                          #@ ok sector
        pos = row["final_pos"]                                          #@ ok final_pos
        direction = "long" if pos > 0 else ("flat" if pos == 0 else "short")
        #@value direction: exactly={"long","flat","short"}
        lines.append(f"{row['ticker']}: {direction} {pos} ({sector})")
    by_sector = summary.groupby("sector")["mtm"].sum().to_dict()        #@ ok mtm
    return lines, by_sector


def run(trades_text, prices_text):
    trades = load_trades(trades_text)
    daily = daily_trades(trades)
    prices = load_prices(prices_text)
    book = build_book(prices, daily)
    port, rets = wide_views(book, prices)
    summary = snapshot(book, prices)
    for probe in (probe_qty, probe_accounts):
        try:
            probe(book)
        except KeyError:
            pass
    return report(summary)


if __name__ == "__main__":
    for t, p in [(TRADES_1, PRICES_1), (TRADES_2, PRICES_2), (TRADES_1, PRICES_2)]:
        print(run(t, p))
#@schema read_csv@L44: always={date,ticker,side,qty,price} only={date,ticker,side,qty,price}
#@schema read_csv@L67: always={date,ticker,close} only={date,ticker,close}

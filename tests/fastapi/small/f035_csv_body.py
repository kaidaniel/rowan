# csv inside handlers: body rows written with DictWriter(fieldnames=…, extrasaction="ignore") and read back (closed rows); raw CSV text read with DictReader (open rows)
import csv
import io
from typing import Any, Dict

from fastapi import FastAPI
from fastapi.testclient import TestClient

app = FastAPI()


def export(rows):
    buf = io.StringIO()
    w = csv.DictWriter(buf, fieldnames=["sku", "qty"], extrasaction="ignore")
    w.writeheader()
    for r in rows:
        w.writerow(r)
    return buf.getvalue()


def reimport(text):
    out = []
    for rec in csv.DictReader(io.StringIO(text)):
        #@row rec: present={sku,qty} definite={sku,qty} closed
        out.append({"sku": rec["sku"], "qty": int(rec["qty"] or 0)})  #@ ok sku ; ok qty
    return out


@app.post("/export")
def export_rows(body: Dict[str, Any]):
    text = export(body["rows"])                      #@ input rows
    back = reimport(text)
    return {"csv": text, "n": len(back)}


def load_prices(text):
    prices = {}
    for rec in csv.DictReader(io.StringIO(text)):
        #@row rec: open
        prices[rec["sku"]] = float(rec["price"])     #@ input sku ; input price
        #@row rec: definite={sku,price} open
    return prices


@app.post("/prices")
def import_prices(body: Dict[str, Any]):
    prices = load_prices(body["text"])               #@ input text
    return {"n": len(prices), "max": max(prices.values()) if prices else None}
#@schema param import_prices.body: always={text} only={text}


if __name__ == "__main__":
    client = TestClient(app)
    assert client.post("/export", json={"rows": [{"sku": "a", "qty": 2, "note": "x"}, {"sku": "b"}]}).json()["n"] == 2
    assert client.post("/export", json={"rows": []}).json()["n"] == 0
    assert client.post("/prices", json={"text": "sku,price\na,1.5\nb,2\n"}).json() == {"n": 2, "max": 2.0}
    assert client.post("/prices", json={"text": "sku,price,cur\na,1,eur\n"}).json()["n"] == 1
    assert client.post("/prices", json={"text": ""}).json()["n"] == 0
    for bad in [{"text": "id,price\na,1\n"}, {"csv": "sku,price\n"}]:
        try:
            client.post("/prices", json=bad)
        except KeyError:
            pass

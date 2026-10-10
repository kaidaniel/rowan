# Order / inventory service over a package: pydantic models (aliases, before-validator, computed fields, exclude_none), Decimal pricing, a repository with Counters, dataclass / Enum events, FastAPI routes driven by a TestClient
from fastapi.testclient import TestClient

from shop import OrderIn, quote
from shop.api import LOG, ORDERS, app
from shop.pricing import register_promo

if __name__ == "__main__":
    # ---------------------------------------------------------------- models and pricing directly
    m = OrderIn.model_validate({"customerId": "c9", "lines": [{"sku": "A1", "price": "1.5"}, {"sku": "C3", "qty": 2, "price": "0.25"}],
                                "shipping": {"street": "Storgata 1", "city": "Oslo", "zip": "0150"}})
    #@row m: present={customer,items,shipping,coupon,note} definite={customer,items,shipping,coupon,note} closed
    n = m.n_items                                    #@value n: exactly={3}
    d = m.model_dump(exclude_none=True)
    #@row d: present={customer,items,shipping,n_items} definite={customer,items,shipping,n_items} absent={coupon,note} closed
    first = d["items"][0]                            #@row first: present={sku,qty,unit_price,line_key} definite={sku,qty,unit_price,line_key} closed
    key0 = first["line_key"]                         #@value key0: exactly={"A1x1"}
    ship = d["shipping"]                             #@row ship: present={street,city,zip_code,country} definite={street,city,zip_code,country} closed
    al = m.model_dump(by_alias=True, include={"customer", "shipping"})
    #@row al: present={customerId,shipping} definite={customerId,shipping} closed
    zipv = al["shipping"]["zip"]                     #@ ok zip
    #@value zipv: exactly={"0150"}
    q0 = quote(m, m.country())
    #@row q0: present={subtotal,tax,total} definite={subtotal,tax,total} absent={discount} closed
    t0 = q0["total"]                                 #@value t0: exactly={"2.50"}
    promo = register_promo({"code": "spring", "rate": "0.2", "min_total": "50"})
    #@row promo: present={code,rate,floor} definite={code,rate,floor} closed
    floor1 = promo["floor"]                          #@value floor1: exactly={"50.00"}
    promo2 = register_promo({"code": "vip", "rate": "0.15"})
    code2 = promo2["code"]                           #@value code2: exactly={"VIP"}
    floor2 = promo2["floor"]                         #@value floor2: exactly={"0"}
    #@schema param register_promo.raw: always={code,rate} only={code,rate}

    # ---------------------------------------------------------------- the HTTP side
    client = TestClient(app)
    r1 = client.post("/orders", json={"customerId": "c1", "items": [{"sku": "A1", "qty": 2, "price": "9.99"}], "coupon": "TEN"})
    s1 = r1.status_code                              #@value s1: exactly={201}
    o1 = r1.json()
    #@row o1: present={order_id,customer,total,status,coupon} definite={order_id,customer,total,status,coupon} absent={tracking} closed
    tot1 = o1["total"]                               #@value tot1: exactly={"22.48"}

    r2 = client.post("/orders", json={"customerId": "c2", "lines": [{"sku": "C3", "qty": 3, "price": "4.10"}, {"sku": "B2", "price": "100"}],
                                      "shipping": {"street": "Hauptstr. 5", "city": "Berlin", "zip": "10115", "country": "DE"}, "note": "gift"})
    o2 = r2.json()
    #@row o2: present={order_id,customer,total,status} definite={order_id,customer,total,status} absent={coupon,tracking} closed
    id2 = o2["order_id"]                             #@value id2: exactly={"ORD-0002"}
    tot2 = o2["total"]                               #@value tot2: exactly={"133.64"}

    r3 = client.post("/orders", json={"customerId": "c1", "items": [{"sku": "B2", "price": "100"}]})
    s3 = r3.status_code                              #@value s3: exactly={409}
    short3 = r3.json()["detail"]["short"]            #@row short3: present={B2} definite={B2} closed
    rejected = LOG.events[-1].data                   #@row rejected: present={short,customer} definite={short,customer} closed
    s4 = client.post("/orders", json={"items": []}).status_code   #@value s4: exactly={422}

    r5 = client.post("/orders", json={"customerId": "c3", "items": [{"sku": "A1", "price": "0.05"}], "coupon": "BOGUS",
                                      "shipping": {"street": "Drottninggatan 2", "city": "Stockholm", "zip": "11151", "country": "SE"}})
    o5 = r5.json()
    #@row o5: present={order_id,customer,total,status,coupon} definite={order_id,customer,total,status,coupon} absent={tracking} closed
    tot5 = o5["total"]                               #@value tot5: exactly={"0.06"}

    g2 = client.get("/orders/ORD-0002").json()       #@row g2: present={order_id,customer,total,status} definite={order_id,customer,total,status} closed
    g9 = client.get("/orders/ORD-0009").status_code  #@value g9: exactly={404}

    sh = client.post("/orders/ORD-0001/ship", json={"carrier": "posten"}).json()
    #@row sh: present={order_id,customer,total,status,coupon,tracking} definite={order_id,customer,total,status,coupon,tracking} closed
    trk = sh["tracking"]                             #@value trk: exactly={"POSTEN-ORD-0001"}
    sh404 = client.post("/orders/ORD-0042/ship", json={"carrier": "ups"}).status_code   #@value sh404: exactly={404}
    #@schema param ship_order.body: always={carrier} only={carrier}

    c1 = client.post("/orders/ORD-0001/cancel").status_code      #@value c1: exactly={409}
    c3 = client.post("/orders/ORD-0003/cancel").json()           #@row c3: present={order_id,status} definite={order_id,status} closed
    last = LOG.last()                                #@row last: present={kind,order_id,data,seq} definite={kind,order_id,data,seq} closed
    seq = last["seq"]                                #@todo value seq: exactly={6}
    stored = ORDERS.orders                           #@row stored: present={ORD-0001,ORD-0002,ORD-0003} definite={ORD-0001,ORD-0002,ORD-0003} closed

    st = client.get("/stats").json()                 #@row st: present={orders,events,top} definite={orders,events,top} closed
    evc = st["events"]                               #@row evc: present={placed,rejected,shipped,cancelled} definite={placed,rejected,shipped,cancelled} closed
    top = st["top"]                                  #@value top: exactly={"c1"}

    rs1 = client.post("/stock", json={"sku": "A1", "qty": "4"}).json()
    #@row rs1: present={sku,stock} definite={sku,stock} absent={ref} closed
    rs2 = client.post("/stock", json={"sku": "B2", "qty": 2, "reason": "return", "ref": "RMA-7"}).json()
    #@row rs2: present={sku,stock,ref} definite={sku,stock,ref} closed
    b2 = rs2["stock"]                                #@value b2: exactly={3}
    #@schema param restock.payload: always={sku,qty} sometimes={ref} only={sku,qty,ref}
    avail = client.get("/stock/A1").json()["available"]   #@value avail: exactly={7}

# pd.json_normalize: nested dicts flatten to dotted names (sep= changes the dot), max_level stops early, record_path unnests a list, meta= / record_prefix=
import pandas as pd


def flat(records):
    df = pd.json_normalize(records)
    #@row df: present={id,"user.name","user.addr.city","user.addr.zip"} definite={id,"user.name","user.addr.city"} absent={user,"user.addr"} closed
    df["user.name"]                                #@ ok "user.name"
    df["user"]                                     #@ missing user


def underscored(records):
    df = pd.json_normalize(records, sep="_")
    #@row df: present={id,user_name,user_addr_city,user_addr_zip} definite={id,user_name,user_addr_city} absent={"user.name"} closed
    lvl = pd.json_normalize(records, max_level=1)
    #@row lvl: present={id,"user.name","user.addr"} definite={id,"user.name","user.addr"} absent={"user.addr.city"} closed
    return lvl["user.addr"]                        #@ ok "user.addr"


def orders(docs):
    items = pd.json_normalize(docs, record_path="items", meta=["order"])
    #@row items: present={sku,qty,order} definite={sku,qty,order} absent={items,cust} closed
    pre = pd.json_normalize(docs, record_path="items", meta=["order", ["cust", "name"]], record_prefix="it_")
    #@row pre: present={it_sku,it_qty,order,"cust.name"} definite={it_sku,it_qty,order,"cust.name"} absent={sku,qty} closed
    return pre["cust.name"]                        #@ ok "cust.name"


if __name__ == "__main__":
    r1 = [{"id": 1, "user": {"name": "a", "addr": {"city": "x"}}}]
    r2 = [{"id": 1, "user": {"name": "a", "addr": {"city": "x"}}},
          {"id": 2, "user": {"name": "b", "addr": {"city": "y", "zip": "01"}}}]
    for r in (r1, r2):
        try:
            flat(r)
        except KeyError:
            pass
        underscored(r)
    d1 = [{"order": 1, "cust": {"name": "a"}, "items": [{"sku": "x", "qty": 1}, {"sku": "y", "qty": 2}]}]
    d2 = d1 + [{"order": 2, "cust": {"name": "b"}, "items": [{"sku": "z", "qty": 3}]}]
    orders(d1)
    orders(d2)

# functools.reduce with a dict accumulator: the reducer receives the accumulator and each record

from functools import reduce


def add_sale(acc, sale):
    acc[sale["region"]] = acc.get(sale["region"], 0) + sale["amount"]
    return acc
#@schema param add_sale.sale: always={region,amount} only={region,amount}


def by_region(sales):
    totals = reduce(add_sale, sales, {})
    return totals


def merge_all(dicts):
    merged = reduce(lambda a, b: {**a, **b}, dicts, {"v": 0})
    #@row merged: present={v,a,b,c} definite={v} closed
    return merged


t = by_region([{"region": "eu", "amount": 3}, {"region": "us", "amount": 4}, {"region": "eu", "amount": 1}])
#@row t: present={eu,us} definite={eu,us} closed
eu = t["eu"]                                       #@value eu: exactly={4}
t2 = by_region([])                                 #@row t2: present={} closed
m = merge_all([{"a": 1}, {"b": 2, "v": 5}])        #@row m: present={v,a,b} definite={v,a,b} closed
m2 = merge_all([{"c": 3}])                         #@row m2: present={v,c} definite={v,c} closed
vv = m["v"]                                        #@value vv: exactly={5}
try:
    t2["eu"]                                       #@ missing eu
except KeyError:
    pass

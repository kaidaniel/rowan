"""A fluent builder (methods returning self) producing query dicts, and a pipeline of stage
objects assembled from a config; records flow through the stages in order.

Expectations were written from the program text before running the engine."""
import sys


class QueryBuilder:
    def __init__(self, table):
        self.parts = {"table": table}
        self.filters = []

    def where(self, column, value):
        self.filters.append({"column": column, "value": value})
        return self

    def limit(self, n):
        self.parts["limit"] = n
        return self

    def order_by(self, column, desc=False):
        self.parts["order"] = column
        if desc:
            self.parts["desc"] = True
        return self

    def build(self):
        q = dict(self.parts)
        q["filters"] = list(self.filters)
        q["sql"] = "select * from " + q["table"]
        return q


q1 = QueryBuilder("users").where("age", 3).limit(10).build()  #@row q1: present={table,limit,filters,sql} definite={table,limit,filters,sql} closed
q2 = QueryBuilder("orders").order_by("total", desc=True).where("x", 1).where("y", 2).build()  #@row q2: present={table,order,desc,filters,sql} definite={table,order,desc,filters,sql} closed
q3 = QueryBuilder("t").order_by("a").build()  #@row q3: present={table,order,filters,sql} definite={table,order,filters,sql} closed
flt = q2["filters"][0]                    #@row flt: present={column,value} definite={column,value} closed

builder = QueryBuilder("items")
builder.limit(5)
parts = builder.parts                     #@row parts: present={table,limit} definite={table,limit} closed
builder.order_by("k", desc=len(sys.argv) > 2)
_ = 0                                     #@row parts: present={table,limit,order,desc} definite={table,limit,order} closed
built = builder.build()                   #@row built: present={table,limit,order,desc,filters,sql} definite={table,limit,order,filters,sql} closed
_ = 0                                     #@row builder: present={parts,filters} definite={parts,filters} closed


class Stage:
    def apply(self, rec):
        return rec


class AddTax(Stage):
    def __init__(self, rate):
        self.rate = rate

    def apply(self, rec):
        rec["tax"] = rec["price"] * self.rate
        return rec


class Discount(Stage):
    def __init__(self, code):
        self.code = code

    def apply(self, rec):
        if rec.get("coupon") == self.code:
            rec["discount"] = 1
        return rec


class Rename(Stage):
    def __init__(self, old, new):
        self.old = old
        self.new = new

    def apply(self, rec):
        rec[self.new] = rec.pop(self.old)
        return rec


class Pipeline:
    def __init__(self, stages):
        self.stages = list(stages)

    def add(self, stage):
        self.stages.append(stage)
        return self

    def run(self, rec):
        for st in self.stages:
            rec = st.apply(rec)
        return rec


def pipeline_from_config(cfg):
    p = Pipeline([AddTax(cfg["tax_rate"])])
    if cfg.get("coupon"):
        p.add(Discount(cfg["coupon"]))
    p.add(Rename("price", "net"))
    return p


# No coupon in the config: AddTax then Rename.
p1 = pipeline_from_config({"tax_rate": 0.2})
out1 = p1.run({"price": 10, "sku": "a"})  #@row out1: present={sku,tax,net} definite={sku,tax,net} closed
out1["price"]                             #@ missing price

# Coupon configured and matching: the Discount stage applies (a comparison of two known
# strings is decided).
p2 = pipeline_from_config({"tax_rate": 0.2, "coupon": "X"})
out2 = p2.run({"price": 10, "sku": "b", "coupon": "X"})  #@row out2: present={sku,coupon,tax,discount,net} definite={sku,coupon,tax,discount,net} closed
out2["net"]                               #@ ok net
out2["discount"]                          #@ ok discount
# Another code on the record: no discount.
out3 = p2.run({"price": 1, "sku": "c", "coupon": "Y"})  #@row out3: present={sku,coupon,tax,net} definite={sku,coupon,tax,net} closed
out3["discount"]                          #@ missing discount
# Unknown code on the record: the comparison is open.
out4 = p2.run({"price": 1, "sku": "d", "coupon": sys.argv[1]})  #@row out4: present={sku,coupon,tax,discount,net} definite={sku,coupon,tax,net} closed
out4["discount"]                          #@ possibly discount

# The stage objects themselves.
first = p2.stages[0]                      #@row first: present={rate} definite={rate} closed
last = p2.stages[2]                       #@row last: present={old,new} definite={old,new} closed

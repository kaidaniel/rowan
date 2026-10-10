# Order ETL: raw dict rows validated by pydantic (aliases, defaults, a field_validator), dumped to records, a pandas frame with a derived column, groupby / agg per region, a summary dict
import pandas as pd
from pydantic import BaseModel, Field, ValidationError, field_validator


class Order(BaseModel):
    order_id: int = Field(alias="id")
    customer: str
    region: str = "eu"
    qty: int = 1
    unit_price: float = Field(alias="price")
    coupon: str | None = None

    @field_validator("customer")
    @classmethod
    def clean_customer(cls, v):
        v = v.strip()
        if not v:
            raise ValueError("empty customer")
        return v.title()


def to_record(order):
    rec = order.model_dump(exclude={"coupon"})
    rec["total"] = round(rec["qty"] * rec["unit_price"], 2)   #@ ok qty ; ok unit_price
    rec["discounted"] = order.coupon is not None
    #@row rec: present={order_id,customer,region,qty,unit_price,total,discounted} definite={order_id,customer,region,qty,unit_price,total,discounted} absent={coupon,id,price} closed
    return rec


def validate_rows(raws):
    good, bad = [], []
    for raw in raws:
        try:
            order = Order.model_validate(raw)
        except ValidationError as e:
            bad.append({"row": raw.get("id"), "errors": len(e.errors())})
            continue
        _ = 0  #@row order: present={order_id,customer,region,qty,unit_price,coupon} definite={order_id,customer,region,qty,unit_price,coupon} closed
        good.append(to_record(order))
    return good, bad


def from_api(payload):
    return to_record(Order.model_validate(payload))
#@schema param from_api.payload: always={id,customer,price} only={id,customer,price}


def legacy_id(order):
    return order.id                              #@ missing id


def build_frame(records):
    df = pd.DataFrame(records)
    df["net"] = df["total"] * (1 - df["discounted"] * 0.1)
    #@row df: present={order_id,customer,region,qty,unit_price,total,discounted,net} definite={order_id,customer,region,qty,unit_price,total,discounted,net} closed
    by_region = df.groupby("region").agg(orders=("order_id", "count"), revenue=("net", "sum"), units=("qty", "sum")).reset_index()
    #@row by_region: present={region,orders,revenue,units} definite={region,orders,revenue,units} absent={net,customer} closed
    return df, by_region


def summarize(df, by_region, bad):
    top = by_region.sort_values("revenue", ascending=False).iloc[0]
    summary = {
        "n_orders": int(len(df)),
        "n_rejected": len(bad),
        "regions": sorted(by_region["region"].tolist()),
        "top_region": top["region"],             #@ ok region
        "revenue": round(float(df["net"].sum()), 2),
    }
    if bad:
        summary["rejected_ids"] = [b["row"] for b in bad]   #@ ok row
    _ = 0  #@row summary: present={n_orders,n_rejected,regions,top_region,revenue,rejected_ids} definite={n_orders,n_rejected,regions,top_region,revenue} closed
    return summary


def run(raws):
    good, bad = validate_rows(raws)
    df, by_region = build_frame(good)
    return summarize(df, by_region, bad)


BATCH_A = [
    {"id": 1, "customer": " ann lee ", "price": 9.5, "qty": 2},
    {"id": "2", "customer": "bob", "region": "us", "price": "20", "coupon": "X10"},
    {"id": 3, "customer": "   ", "price": 1.0},
    {"customer": "cyd", "price": 3.0},
    {"id": 5, "customer": "dee", "region": "us", "price": 4.0, "qty": 3, "note": "extra"},
]
BATCH_B = [{"id": 10, "customer": "eve", "region": "apac", "price": 7.25, "qty": 4}]

if __name__ == "__main__":
    good_a, bad_a = validate_rows(BATCH_A)
    n_bad = len(bad_a)                           #@value n_bad: exactly={2}
    ids = tuple(b["row"] for b in bad_a)         #@value ids: exactly={(3,None)}
    b0 = bad_a[0]                                #@row b0: present={row,errors} definite={row,errors} closed
    first = good_a[0]["customer"]                #@value first: exactly={"Ann Lee"}
    second = good_a[1]
    #@row second: present={order_id,customer,region,qty,unit_price,total,discounted} definite={order_id,customer,region,qty,unit_price,total,discounted} closed
    tot = second["total"]                        #@value tot: exactly={20.0}
    disc = second["discounted"]                  #@value disc: exactly={True}
    s_a = run(BATCH_A)
    s_b = run(BATCH_B)
    #@row s_b: present={n_orders,n_rejected,regions,top_region,revenue} definite={n_orders,n_rejected,regions,top_region,revenue} absent={rejected_ids} closed
    api = from_api({"id": 7, "customer": "fay", "price": 2.5, "region": "us"})
    reg = api["region"]                          #@value reg: exactly={us}
    try:
        from_api({"id": 8, "customer": "", "price": 1.0})
    except ValidationError:
        pass
    try:
        legacy_id(Order.model_validate(BATCH_B[0]))
    except AttributeError:
        pass

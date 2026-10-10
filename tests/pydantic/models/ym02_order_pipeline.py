# order pipeline: JSON order documents -> nested models (line items, discriminated payment), computed totals, a summary dump, a JSON round trip and a strict export model
import json
from decimal import Decimal
from typing import Annotated, Literal, Union

from pydantic import BaseModel, ConfigDict, Field, TypeAdapter, ValidationError, computed_field, field_serializer, field_validator


class LineItem(BaseModel):
    sku: str
    qty: int = Field(1, gt=0)
    unit_price: Decimal = Field(alias="price")

    @computed_field
    @property
    def subtotal(self) -> Decimal:
        return self.unit_price * self.qty


class Card(BaseModel):
    method: Literal["card"]
    last4: str
    brand: str = "visa"


class Invoice(BaseModel):
    method: Literal["invoice"]
    due_days: int = 30


class Voucher(BaseModel):
    method: Literal["voucher"]
    code: str
    value: Decimal


Payment = Annotated[Union[Card, Invoice, Voucher], Field(discriminator="method")]


class Order(BaseModel):
    order_id: str = Field(alias="id")
    customer: str
    items: list[LineItem]
    payment: Payment
    notes: str | None = None
    shipping: Decimal = Decimal("0")

    @field_validator("customer")
    @classmethod
    def tidy(cls, v):
        return v.strip()

    @computed_field
    @property
    def total(self) -> Decimal:
        s = sum((i.subtotal for i in self.items), Decimal("0")) + self.shipping
        if isinstance(self.payment, Voucher):
            s = max(Decimal("0"), s - self.payment.value)
        return s

    @field_serializer("shipping")
    def ship_str(self, v):
        return str(v)


class Export(BaseModel):
    model_config = ConfigDict(extra="forbid")
    id: str
    customer: str
    total: str
    method: str
    lines: int


ORDERS = TypeAdapter(list[Order])


def parse_orders(text):
    raw = json.loads(text)
    orders = ORDERS.validate_python(raw)
    for o in orders:
        #@row o: present={order_id,customer,items,payment,notes,shipping} definite={order_id,customer,items,payment,notes,shipping} absent={id,total} closed
        pay = o.payment
        #@row pay: present={method,last4,brand,due_days,code,value} definite={method} closed
        for it in o.items:
            #@row it: present={sku,qty,unit_price} definite={sku,qty,unit_price} absent={price,subtotal} closed
            p = it.unit_price                    #@ ok unit_price
    return orders


def payment_label(o):
    pay = o.payment
    if isinstance(pay, Card):
        return f"{pay.brand} ****{pay.last4}"    #@ ok brand ; ok last4
    if isinstance(pay, Voucher):
        return f"voucher {pay.code}"             #@ ok code
    return f"invoice +{pay.due_days}d"           #@ ok due_days


def card_digits(o):
    return o.payment.last4                       #@ possibly last4


def summary(o):
    d = o.model_dump(include={"order_id", "customer", "total", "payment"})
    #@row d: present={order_id,customer,total,payment} definite={order_id,customer,total,payment} absent={items,notes} closed
    pay = d["payment"]                           #@ ok payment
    #@row pay: present={method,last4,brand,due_days,code,value} definite={method} closed
    return d


def wire(o):
    w = json.loads(o.model_dump_json(by_alias=True, exclude_none=True))
    #@row w: present={id,customer,items,payment,notes,shipping,total} definite={id,customer,items,payment,shipping,total} absent={order_id} closed
    for line in w["items"]:
        #@row line: present={sku,qty,price,subtotal} definite={sku,qty,price,subtotal} absent={unit_price} closed
        q = line["qty"]                          #@ ok qty
    ship = w["shipping"]
    return w


def export(o):
    e = Export(id=o.order_id, customer=o.customer, total=str(o.total), method=o.payment.method, lines=len(o.items))
    #@row e: present={id,customer,total,method,lines} definite={id,customer,total,method,lines} closed
    return e.model_dump()


def reprice(o, factor):
    items = [it.model_copy(update={"unit_price": it.unit_price * factor}) for it in o.items]
    o2 = o.model_copy(update={"items": items})
    #@row o2: present={order_id,customer,items,payment,notes,shipping} definite={order_id,customer,items,payment,notes,shipping} closed
    return o2.total


TEXT = json.dumps([
    {"id": "o1", "customer": " ann ", "items": [{"sku": "a", "price": "2.50", "qty": 2}], "payment": {"method": "card", "last4": "4242"}},
    {"id": "o2", "customer": "bo", "items": [{"sku": "b", "price": "10"}, {"sku": "c", "price": "1.25", "qty": 4}], "payment": {"method": "invoice"}, "shipping": "4.95", "notes": "leave at door"},
    {"id": "o3", "customer": "cy", "items": [{"sku": "d", "price": "8"}], "payment": {"method": "voucher", "code": "X", "value": "10"}},
])
BAD_TEXT = json.dumps([{"id": "o4", "customer": "dee", "items": [{"sku": "e", "price": "1"}], "payment": {"method": "card"}}])

orders = parse_orders(TEXT)
first = orders[0]
cust = first.customer
#@value cust: exactly={"ann"}
tot = str(first.total)
#@value tot: exactly={"5.00"}
lab = payment_label(first)
#@value lab: exactly={"visa ****4242"}
third_total = str(orders[2].total)
#@value third_total: exactly={"0"}
w2 = wire(orders[1])
ship2 = w2["shipping"]
#@value ship2: exactly={"4.95"}
ex = export(orders[1])
#@row ex: present={id,customer,total,method,lines} definite={id,customer,total,method,lines} closed
n_lines = ex["lines"]
#@value n_lines: exactly={2}

if __name__ == "__main__":
    for o in orders:
        payment_label(o)
        summary(o)
        wire(o)
        export(o)
        reprice(o, Decimal("1.1"))
        try:
            card_digits(o)
        except AttributeError:
            pass
    try:
        parse_orders(BAD_TEXT)
    except ValidationError:
        pass

#@schema json.loads@L79['*']: always={id,customer,items,payment} only={id,customer,items,payment}
#@schema json.loads@L79['*']['items']['*']: always={sku,price} only={sku,price}
#@schema json.loads@L79['*']['payment']: always={method} sometimes={last4,code,value} only={method,last4,code,value}

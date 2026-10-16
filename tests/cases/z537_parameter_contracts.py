# A parameter's contract: what the function reads of it for any call (always, or only under a
# test of the other parameters), kept apart from what each of the program's calls passes and
# whether that has it. The hovers over `row` and `order` in their `def`s are checked in
# vscode-rowan/test.js; the contracts are the `param F.x` schemas.
import sys


def make_user(row, admin=False):
    user = {"id": row["id"], "name": row["name"].strip()}
    if admin:
        user["since"] = row["since"]
    return user
#@schema param make_user.row: always={id,name} sometimes={since} only={id,name,since}
#@only param make_user.row: since if admin


def total(order):
    return sum(item["qty"] * item["price"] for item in order["items"])
#@schema param total.order: always={items} only={items}
#@schema param total.order['items']['*']: always={qty,price} only={qty,price}


a = make_user({"id": 1, "name": " ann "})
b = make_user({"id": 2, "name": "bob"}, admin=len(sys.argv) > 1)
t = total({"items": [{"qty": 2, "price": 1.5}]})
try:
    u = total({"items": [{"qty": 1}]})
except KeyError:
    u = None
#@value t: exactly={3.0}

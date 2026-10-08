"""Annotations as declared rows: TypedDict, dataclasses, Optional / Union / forward refs,
typed parameters of entry points, Any fields."""
from dataclasses import dataclass
from typing import Any, Optional, TypedDict, Union


class Address(TypedDict):
    street: str
    city: str


class User(TypedDict):
    id: int
    address: Address


@dataclass
class Account:
    user: User
    balance: float
    meta: Any = None


def city_of(u: User) -> str:
    return u["address"]["city"]          #@ ok address ; ok city


def street(a: Optional["Address"]):
    if a is None:
        return ""
    return a["street"]                   #@ ok street


def zipcode(a: Union[Address, None]):
    return a["zip"]                      #@ missing zip


def balance_of(acc: Account):
    acc.balance                          #@ ok balance
    acc.user["id"]                       #@ ok user ; ok id
    acc.meta["anything"]                 #@ none anything
    acc.nothing                          #@ missing nothing
    return acc.balance


def untyped(x):
    return x["k"]                        #@ input k


u: User = {"id": 1, "address": {"street": "s", "city": "c"}}
city_of(u)                               #@ ok city
acct = Account(u, 1.0)
balance_of(acct)
#@schema param untyped.x: always={k}
